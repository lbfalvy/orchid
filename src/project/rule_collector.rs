use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;

use crate::expression::Rule;
use crate::parse::{self, FileEntry};
use crate::utils::Cache;

use super::name_resolver::NameResolver;
use super::module_error::ModuleError;
use super::prefix::prefix_expr;
use super::loaded::Loaded;

type ParseResult<T, ELoad> = Result<T, ModuleError<ELoad>>; 

#[derive(Debug, Clone)]
pub struct Module {
    pub rules: Vec<Rule>,
    pub exports: Vec<String>,
    pub references: Vec<Vec<String>>
}

pub fn rule_collector<F: 'static, ELoad>(
    mut load_mod: F,
    prelude: Vec<String>
// ) -> impl FnMut(Vec<String>) -> Result<&'a Vec<super::Rule>, ParseError<ELoad>> + 'a
) -> Cache<'static, Vec<String>, Result<Vec<super::Rule>, ModuleError<ELoad>>>
where
    F: FnMut(Vec<String>) -> Result<Loaded, ELoad>,
    ELoad: Clone + Debug
{
    // Map paths to a namespace with name list (folder) or module with source text (file)
    let loaded = Rc::new(Cache::new(move |path: Vec<String>, _|
         -> ParseResult<Loaded, ELoad> {
        load_mod(path).map_err(ModuleError::Load)
    }));
    // Map names to the longest prefix that points to a valid module
    let modname = Rc::new(Cache::new({
        let loaded = Rc::clone(&loaded);
        move |symbol: Vec<String>, _| -> Result<Vec<String>, Vec<ModuleError<ELoad>>> {
            let mut errv: Vec<ModuleError<ELoad>> = Vec::new();
            let reg_err = |e, errv: &mut Vec<ModuleError<ELoad>>| {
                errv.push(e);
                if symbol.len() == errv.len() { Err(errv.clone()) }
                else { Ok(()) }
            };
            loop {
                let (path, _) = symbol.split_at(symbol.len() - errv.len());
                let pathv = path.to_vec();
                match loaded.try_find(&pathv) {
                    Ok(imports) => match imports.as_ref() {
                        Loaded::Module(_) => break Ok(pathv.clone()),
                        _ => reg_err(ModuleError::None, &mut errv)?
                    },
                    Err(err) => reg_err(err, &mut errv)?
                }
            }
        }
    }));
    // Preliminarily parse a file, substitution rules and imports are valid
    let preparsed = Rc::new(Cache::new({
        let loaded = Rc::clone(&loaded);
        let prelude2 = prelude.clone();
        move |path: Vec<String>, _| -> ParseResult<Vec<FileEntry>, ELoad> {
            let loaded = loaded.try_find(&path)?;
            if let Loaded::Module(source) = loaded.as_ref() {
                Ok(parse::parse(&prelude2, source.as_str())?)
            } else {Err(ModuleError::None)}
        }
    }));
    // Collect all toplevel names exported from a given file
    let exports = Rc::new(Cache::new({
        let loaded = Rc::clone(&loaded);
        let preparsed = Rc::clone(&preparsed);
        move |path: Vec<String>, _| -> ParseResult<Vec<String>, ELoad> {
            let loaded = loaded.try_find(&path)?;
            if let Loaded::Namespace(names) = loaded.as_ref() {
                return Ok(names.clone());
            }
            let preparsed = preparsed.try_find(&path)?;
            Ok(parse::exported_names(&preparsed)
                .into_iter()
                .map(|n| n[0].clone())
                .collect())
        }
    }));
    // Collect all toplevel names imported by a given file
    let imports = Rc::new(Cache::new({
        let preparsed = Rc::clone(&preparsed);
        let exports = Rc::clone(&exports);
        move |path: Vec<String>, _| -> ParseResult<HashMap<String, Vec<String>>, ELoad> {
            let entv = preparsed.try_find(&path)?.clone();
            let import_entries = parse::imports(entv.iter());
            let mut imported_symbols: HashMap<String, Vec<String>> = HashMap::new();
            for imp in import_entries {
                let export = exports.try_find(&imp.path)?;
                if let Some(ref name) = imp.name {
                    if export.contains(&name) {
                        imported_symbols.insert(name.clone(), imp.path.clone());
                    }
                } else {
                    for exp in export.as_ref() {
                        imported_symbols.insert(exp.clone(), imp.path.clone());
                    }
                }
            }
            Ok(imported_symbols)
        }
    }));
    // Final parse, operators are correctly separated
    let parsed = Rc::new(Cache::new({
        let preparsed = Rc::clone(&preparsed);
        let imports = Rc::clone(&imports);
        let loaded = Rc::clone(&loaded);
        move |path: Vec<String>, _| -> ParseResult<Vec<FileEntry>, ELoad> {
            let imported_ops: Vec<String> =
                imports.try_find(&path)?
                .keys()
                .chain(prelude.iter())
                .filter(|s| parse::is_op(s))
                .cloned()
                .collect();
            // let parser = file_parser(&prelude, &imported_ops);
            let pre = preparsed.try_find(&path)?;
            if let Loaded::Module(source) = loaded.try_find(&path)?.as_ref() {
                Ok(parse::reparse(&imported_ops, source.as_str(), &pre)?)
            } else { Err(ModuleError::None) }
        }
    }));
    let mut name_resolver = NameResolver::new({
        let modname = Rc::clone(&modname);
        move |path| {
            Some(modname.try_find(path).ok()?.as_ref().clone())
        }
    }, {
        let imports = Rc::clone(&imports);
        move |path| {
            imports.try_find(path).map(|f| f.as_ref().clone())
        }
    });
    // Turn parsed files into a bag of rules and a list of toplevel export names
    let resolved = Rc::new(Cache::new({
        let parsed = Rc::clone(&parsed);
        let exports = Rc::clone(&exports);
        let imports = Rc::clone(&imports);
        let modname = Rc::clone(&modname);
        move |path: Vec<String>, _| -> ParseResult<Module, ELoad> {
            let module = Module {
                rules: parsed.try_find(&path)?
                    .iter()
                    .filter_map(|ent| {
                        if let FileEntry::Rule(Rule{source, prio, target}, _) = ent {
                            Some(Rule {
                                source: source.iter().map(|ex| prefix_expr(ex, &path)).collect(),
                                target: target.iter().map(|ex| prefix_expr(ex, &path)).collect(),
                                prio: *prio,
                            })
                        } else { None }
                    })
                    .map(|rule| Ok(super::Rule {
                        source: rule.source.iter()
                            .map(|ex| name_resolver.process_expression(ex))
                            .collect::<Result<Vec<_>, _>>()?,
                        target: rule.target.iter()
                            .map(|ex| name_resolver.process_expression(ex))
                            .collect::<Result<Vec<_>, _>>()?,
                        // source: name_resolver.process_expression(&rule.source)?,
                        // target: name_resolver.process_expression(&rule.target)?,
                        ..rule
                    }))
                    .collect::<ParseResult<Vec<super::Rule>, ELoad>>()?,
                exports: exports.try_find(&path)?.as_ref().clone(),
                references: imports.try_find(&path)?
                    .values()
                    .filter_map(|imps| {
                        modname.try_find(&imps).ok().map(|r| r.as_ref().clone())
                    })
                    .collect()
            };
            Ok(module)
        }
    }));
    let all_rules = Cache::new({
        let resolved = Rc::clone(&resolved);
        move |path: Vec<String>, _| -> ParseResult<Vec<super::Rule>, ELoad> {
            let mut processed: HashSet<Vec<String>> = HashSet::new();
            let mut rules: Vec<super::Rule> = Vec::new();
            let mut pending: VecDeque<Vec<String>> = VecDeque::new();
            pending.push_back(path);
            while let Some(el) = pending.pop_front() {
                let resolved = resolved.try_find(&el)?;
                processed.insert(el.clone());
                pending.extend(
                    resolved.references.iter()
                        .filter(|&v| !processed.contains(v))
                        .cloned() 
                );
                rules.extend(
                    resolved.rules.iter().cloned()
                )
            };
            Ok(rules)
        }
    });
    return all_rules; 
}
