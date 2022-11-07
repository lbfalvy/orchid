use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;

use mappable_rc::Mrc;

use crate::ast::Rule;
use crate::parse::{self, FileEntry};
use crate::utils::{Cache, mrc_derive, to_mrc_slice};

use super::name_resolver::NameResolver;
use super::module_error::ModuleError;
use super::prefix::prefix_expr;
use super::loaded::Loaded;

type ParseResult<T, ELoad> = Result<T, ModuleError<ELoad>>; 

#[derive(Debug, Clone)]
pub struct Module {
    pub rules: Vec<Rule>,
    pub exports: Vec<String>,
    pub references: Vec<Mrc<[String]>>
}

pub type RuleCollectionResult<ELoad> = Result<Vec<super::Rule>, ModuleError<ELoad>>;

pub fn rule_collector<F: 'static, ELoad>(
    load_mod: F,
    prelude: Vec<String>
) -> Cache<'static, Mrc<[String]>, RuleCollectionResult<ELoad>>
where
    F: FnMut(Mrc<[String]>) -> Result<Loaded, ELoad>,
    ELoad: Clone + Debug
{
    let load_mod_rc = RefCell::new(load_mod);
    // Map paths to a namespace with name list (folder) or module with source text (file)
    let loaded = Rc::new(Cache::new(move |path: Mrc<[String]>, _|
         -> ParseResult<Loaded, ELoad> {
        (load_mod_rc.borrow_mut())(path).map_err(ModuleError::Load)
    }));
    // Map names to the longest prefix that points to a valid module
    // At least one segment must be in the prefix, and the prefix must not be the whole name 
    let modname = Rc::new(Cache::new({
        let loaded = Rc::clone(&loaded);
        move |symbol: Mrc<[String]>, _| -> Result<Mrc<[String]>, Vec<ModuleError<ELoad>>> {
            let mut errv: Vec<ModuleError<ELoad>> = Vec::new();
            let reg_err = |e, errv: &mut Vec<ModuleError<ELoad>>| {
                errv.push(e);
                if symbol.len() == errv.len() { Err(errv.clone()) }
                else { Ok(()) }
            };
            loop {
                let path = mrc_derive(&symbol, |s| &s[..s.len() - errv.len() - 1]);
                match loaded.try_find(&path) {
                    Ok(imports) => match imports.as_ref() {
                        Loaded::Module(_) => break Ok(path),
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
        move |path: Mrc<[String]>, _| -> ParseResult<Vec<FileEntry>, ELoad> {
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
        move |path: Mrc<[String]>, _| -> ParseResult<Vec<String>, ELoad> {
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
        move |path: Mrc<[String]>, _| -> ParseResult<HashMap<String, Mrc<[String]>>, ELoad> {
            let entv = preparsed.try_find(&path)?;
            let import_entries = parse::imports(entv.iter());
            let mut imported_symbols: HashMap<String, Mrc<[String]>> = HashMap::new();
            for imp in import_entries {
                let export = exports.try_find(&imp.path)?;
                if let Some(ref name) = imp.name {
                    if export.contains(name) {
                        imported_symbols.insert(name.clone(), Mrc::clone(&imp.path));
                    }
                } else {
                    for exp in export.as_ref() {
                        imported_symbols.insert(exp.clone(), Mrc::clone(&imp.path));
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
        move |path: Mrc<[String]>, _| -> ParseResult<Vec<FileEntry>, ELoad> {
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
    let name_resolver_rc = RefCell::new(NameResolver::new({
        let modname = Rc::clone(&modname);
        move |path| {
            Some(modname.try_find(&path).ok()?.as_ref().clone())
        }
    }, {
        let imports = Rc::clone(&imports);
        move |path| {
            imports.try_find(&path).map(|f| f.as_ref().clone())
        }
    }));
    // Turn parsed files into a bag of rules and a list of toplevel export names
    let resolved = Rc::new(Cache::new({
        let parsed = Rc::clone(&parsed);
        let exports = Rc::clone(&exports);
        let imports = Rc::clone(&imports);
        let modname = Rc::clone(&modname);
        move |path: Mrc<[String]>, _| -> ParseResult<Module, ELoad> {
            let mut name_resolver = name_resolver_rc.borrow_mut();
            let module = Module {
                rules: parsed.try_find(&path)?
                    .iter()
                    .filter_map(|ent| {
                        if let FileEntry::Rule(Rule{source, prio, target}, _) = ent {
                            Some(Rule {
                                source: source.iter()
                                    .map(|ex| {
                                        prefix_expr(ex, Mrc::clone(&path))
                                    }).collect(),
                                target: target.iter().map(|ex| {
                                        prefix_expr(ex, Mrc::clone(&path))
                                    }).collect(),
                                prio: *prio,
                            })
                        } else { None }
                    })
                    .map(|rule| Ok(super::Rule {
                        source: to_mrc_slice(rule.source.iter()
                            .map(|ex| name_resolver.process_expression(ex))
                            .collect::<Result<Vec<_>, _>>()?),
                        target: to_mrc_slice(rule.target.iter()
                            .map(|ex| name_resolver.process_expression(ex))
                            .collect::<Result<Vec<_>, _>>()?),
                        ..rule
                    }))
                    .collect::<ParseResult<Vec<super::Rule>, ELoad>>()?,
                exports: exports.try_find(&path)?.as_ref().clone(),
                references: imports.try_find(&path)?
                    .values()
                    .filter_map(|imps| {
                        modname.try_find(imps).ok().map(|r| r.as_ref().clone())
                    })
                    .collect()
            };
            Ok(module)
        }
    }));
    Cache::new({
        let resolved = Rc::clone(&resolved);
        move |path: Mrc<[String]>, _| -> ParseResult<Vec<super::Rule>, ELoad> {
            // Breadth-first search
            let mut processed: HashSet<Mrc<[String]>> = HashSet::new();
            let mut rules: Vec<super::Rule> = Vec::new();
            let mut pending: VecDeque<Mrc<[String]>> = VecDeque::new();
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
    }) 
}
