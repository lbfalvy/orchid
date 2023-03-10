use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;

use itertools::Itertools;
use mappable_rc::Mrc;

use crate::ast::Rule;
use crate::parse::{self, FileEntry};
use crate::utils::{Cache, mrc_derive, to_mrc_slice, one_mrc_slice};

use super::name_resolver::NameResolver;
use super::module_error::ModuleError;
use super::prefix::prefix_expr;
use super::loading::{Loaded, Loader, LoadingError};
use crate::parse::Import;

type ParseResult<T> = Result<T, ModuleError<LoadingError>>; 

#[derive(Debug, Clone)]
pub struct Module {
  pub rules: Vec<Rule>,
  pub exports: Vec<String>,
  pub references: HashSet<Mrc<[String]>>
}

pub type RuleCollectionResult = Result<Vec<super::Rule>, ModuleError<LoadingError>>;

pub fn rule_collector<F: 'static>(
  load_mod: F
) -> Cache<'static, Mrc<[String]>, RuleCollectionResult>
where F: Loader
{
  let load_mod_rc = RefCell::new(load_mod);
  // Map paths to a namespace with name list (folder) or module with source text (file)
  let loaded = Rc::new(Cache::new(move |path: Mrc<[String]>, _| -> ParseResult<Loaded> {
    load_mod_rc.borrow_mut().load(&path.iter().map(|s| s.as_str()).collect_vec()).map_err(ModuleError::Load)
  }));
  // Map names to the longest prefix that points to a valid module
  // At least one segment must be in the prefix, and the prefix must not be the whole name 
  let modname = Rc::new(Cache::new({
    let loaded = loaded.clone();
    move |symbol: Mrc<[String]>, _| -> Result<Mrc<[String]>, Vec<ModuleError<LoadingError>>> {
      let mut errv: Vec<ModuleError<LoadingError>> = Vec::new();
      let reg_err = |e, errv: &mut Vec<ModuleError<LoadingError>>| {
        errv.push(e);
        if symbol.len() == errv.len() { Err(errv.clone()) }
        else { Ok(()) }
      };
      loop {
        let path = mrc_derive(&symbol, |s| &s[..s.len() - errv.len() - 1]);
        match loaded.try_find(&path) {
          Ok(imports) => match imports.as_ref() {
            Loaded::Module(_) | Loaded::External(_) => break Ok(path),
            Loaded::Namespace(_) => reg_err(ModuleError::None, &mut errv)?
          },
          Err(err) => reg_err(err, &mut errv)?
        }
      }
    }
  }));
  // Preliminarily parse a file, substitution rules and imports are valid
  let prelude_path = one_mrc_slice("prelude".to_string());
  let preparsed = Rc::new(Cache::new({
    let loaded = loaded.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<Vec<FileEntry>> {
      let loaded = loaded.try_find(&path)?;
      match loaded.as_ref() {
        Loaded::Module(source) => {
          let mut entv = parse::parse(&[] as &[&str], source.as_str())?;
          if !entv.iter().any(|ent| if let FileEntry::Import(imps) = ent {
            imps.iter().any(|imp| imp.path.starts_with(&prelude_path))
          } else {false}) && path != prelude_path {
            entv.push(FileEntry::Import(vec![Import{
              name: None, path: Mrc::clone(&prelude_path)
            }]))
          }
          Ok(entv)
        }
        Loaded::External(ast) => Ok(ast.clone()),
        Loaded::Namespace(_) => Err(ModuleError::None),
      }
    }
  }));
  // Collect all toplevel names exported from a given file
  let exports = Rc::new(Cache::new({
    let loaded = loaded.clone();
    let preparsed = preparsed.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<Vec<String>> {
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
    let preparsed = preparsed.clone();
    let exports = exports.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<HashMap<String, Mrc<[String]>>> {
      let entv = preparsed.try_find(&path)?;
      let import_entries = parse::imports(entv.iter());
      let mut imported_symbols: HashMap<String, Mrc<[String]>> = HashMap::new();
      for imp in import_entries {
        let export = exports.try_find(&imp.path)?;
        if let Some(ref name) = imp.name {
          if export.contains(name) {
            imported_symbols.insert(name.clone(), Mrc::clone(&imp.path));
          } else {panic!("{:?} doesn't export {}", imp.path, name)}
        } else {
          for exp in export.as_ref() {
            imported_symbols.insert(exp.clone(), Mrc::clone(&imp.path));
          }
        }
      }
      println!("Imports for {:?} are {:?}", path.as_ref(), imported_symbols);
      Ok(imported_symbols)
    }
  }));
  // Final parse, operators are correctly separated
  let parsed = Rc::new(Cache::new({
    let preparsed = preparsed.clone();
    let imports = imports.clone();
    let loaded = loaded.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<Vec<FileEntry>> {
      let imported_ops: Vec<String> =
        imports.try_find(&path)?
        .keys()
        .filter(|s| parse::is_op(s))
        .cloned()
        .collect();
      // let parser = file_parser(&prelude, &imported_ops);
      let pre = preparsed.try_find(&path)?;
      match loaded.try_find(&path)?.as_ref() {
        Loaded::Module(source) => Ok(parse::reparse(&imported_ops, source.as_str(), &pre)?),
        Loaded::External(ast) => Ok(ast.clone()),
        Loaded::Namespace(_) => Err(ModuleError::None)
      }
    }
  }));
  let name_resolver_rc = RefCell::new(NameResolver::new({
    let modname = modname.clone();
    move |path| {
      Some(modname.try_find(&path).ok()?.as_ref().clone())
    }
  }, {
    let imports = imports.clone();
    move |path| {
      imports.try_find(&path).map(|f| f.as_ref().clone())
    }
  }));
  // Turn parsed files into a bag of rules and a list of toplevel export names
  let resolved = Rc::new(Cache::new({
    let parsed = parsed.clone();
    let exports = exports.clone();
    let imports = imports.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<Module> {
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
          .map(|Rule{ source, target, prio }| Ok(super::Rule {
            source: to_mrc_slice(source.iter()
              .map(|ex| name_resolver.process_expression(ex))
              .collect::<Result<Vec<_>, _>>()?),
            target: to_mrc_slice(target.iter()
              .map(|ex| name_resolver.process_expression(ex))
              .collect::<Result<Vec<_>, _>>()?),
            prio
          }))
          .collect::<ParseResult<Vec<super::Rule>>>()?,
        exports: exports.try_find(&path)?.as_ref().clone(),
        references: imports.try_find(&path)?
          .values().cloned().collect()
      };
      Ok(module)
    }
  }));
  Cache::new({
    let resolved = resolved.clone();
    move |path: Mrc<[String]>, _| -> ParseResult<Vec<super::Rule>> {
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
        );
      };
      Ok(rules)
    }
  }) 
}
