use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use itertools::Itertools;
use lasso::Spur;

use crate::ast::Rule;
use crate::parse;
use crate::representations::sourcefile::{FileEntry, exported_names, imports};
use crate::utils::Cache;

use super::name_resolver::NameResolver;
use super::module_error::ModuleError;
use super::prefix::prefix_expr;
use super::loading::{Loaded, Loader, LoadingError};

type ParseResult<T> = Result<T, ModuleError<LoadingError>>; 

#[derive(Clone)]
pub struct Module {
  pub rules: Vec<Rule>,
  pub exports: Vec<Spur>,
  pub references: HashSet<Rc<Vec<Spur>>>
}

pub type RuleCollectionResult = Result<Vec<super::Rule>, ModuleError<LoadingError>>;

pub fn rule_collector<'a, F: 'a, G: 'a, H: 'a>(
  intern: &'a G, deintern: &'a H,
  load_mod: F
) -> Cache<'static, Rc<Vec<Spur>>, RuleCollectionResult>
where F: Loader, G: Fn(&str) -> Spur, H: Fn(Spur) -> &'a str
{
  let load_mod_rc = RefCell::new(load_mod);
  // Map paths to a namespace with name list (folder) or module with source text (file)
  let loaded = Cache::rc(move |path: Rc<Vec<Spur>>, _| -> ParseResult<Rc<Loaded>> {
    let load_mod = load_mod_rc.borrow_mut();
    let spath = path.iter().cloned().map(deintern).collect_vec();
    load_mod.load(&spath).map(Rc::new).map_err(ModuleError::Load)
  });
  // Map names to the longest prefix that points to a valid module
  // At least one segment must be in the prefix, and the prefix must not be the whole name 
  let modname = Cache::rc({
    let loaded = loaded.clone();
    move |symbol: Rc<Vec<Spur>>, _| -> Result<Rc<Vec<Spur>>, Rc<Vec<ModuleError<LoadingError>>>> {
      let mut errv: Vec<ModuleError<LoadingError>> = Vec::new();
      let reg_err = |e, errv: &mut Vec<ModuleError<LoadingError>>| {
        errv.push(e);
        if symbol.len() == errv.len() { Err(Rc::new(errv.clone())) }
        else { Ok(()) }
      };
      loop {
        // TODO: this should not live on the heap
        let path = Rc::new(symbol.iter()
          .take(symbol.len() - errv.len() - 1)
          .cloned()
          .collect_vec());
        match loaded.find(&path).as_ref() {
          Ok(imports) => match imports.as_ref() {
            Loaded::Source(_) | Loaded::AST(_) => break Ok(path),
          },
          Err(err) => reg_err(err.clone(), &mut errv)?
        }
      }
    }
  });
  // Preliminarily parse a file, substitution rules and imports are valid
  let preparsed = Rc::new(Cache::new({
    // let prelude_path = vec!["prelude".to_string()];
    // let interned_prelude_path = Rc::new(
    //   prelude_path.iter()
    //   .map(|s| intern(s.as_str()))
    //   .collect_vec()
    // );
    let loaded = loaded.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Vec<FileEntry>> {
      let loaded = loaded.find(&path)?;
      match loaded.as_ref() {
        Loaded::Source(source) => {
          let mut entv = parse::parse(&[] as &[&str], source.as_str(), intern)?;
          // if path != interned_prelude_path {
          //   entv.push(FileEntry::Import(vec![Import{
          //     name: None, path: prelude_path
          //   }]))
          // }
          Ok(entv)
        }
        Loaded::AST(ast) => Ok(ast.clone()),
      }
    }
  }));
  // Collect all toplevel names exported from a given file
  let exports = Rc::new(Cache::new({
    let loaded = loaded.clone();
    let preparsed = preparsed.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Vec<Spur>> {
      let loaded = loaded.find(&path)?;
      let preparsed = preparsed.find(&path)?;
      Ok(exported_names(&preparsed)
        .into_iter()
        .map(|n| n[0].clone())
        .collect())
    }
  }));
  // Collect all toplevel names imported by a given file
  let imports = Rc::new(Cache::new({
    let preparsed = preparsed.clone();
    let exports = exports.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Rc<HashMap<Spur, Rc<Vec<Spur>>>>> {
      let entv = preparsed.find(&path)?;
      let import_entries = imports(entv.iter());
      let mut imported_symbols = HashMap::<Spur, Rc<Vec<Spur>>>::new();
      for imp in import_entries {
        let export_list = exports.find(&path)?;
        if let Some(ref name) = imp.name {
          if export_list.contains(name) {
            imported_symbols.insert(name.clone(), imp.path.clone());
          } else {
            panic!("{:?} doesn't export {}", imp.path, deintern(*name))
          }
        } else {
          for exp in export_list {
            imported_symbols.insert(exp, imp.path.clone());
          }
        }
      }
      // println!("Imports for {:?} are {:?}", path.as_ref(), imported_symbols);
      Ok(Rc::new(imported_symbols))
    }
  }));
  // Final parse, operators are correctly separated
  let parsed = Rc::new(Cache::new({
    let preparsed = preparsed.clone();
    let imports = imports.clone();
    let loaded = loaded.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Vec<FileEntry>> {
      let imported_ops: Vec<String> =
        imports.find(&path)?
          .keys()
          .map(|s| deintern(*s).to_string())
          .filter(|s| parse::is_op(s))
          .collect();
      let pre = preparsed.find(&path)?;
      match loaded.find(&path)?.as_ref() {
        Loaded::Source(source) => Ok(parse::reparse(
          &imported_ops, source.as_str(), &pre, intern
        )?),
        Loaded::AST(ast) => Ok(ast.clone()),
      }
    }
  }));
  let name_resolver = NameResolver::new({
    let modname = modname.clone();
    move |path| {
      let modname = modname.find(&path).ok()?;
      let symname = Rc::new(path[modname.len()..].to_vec());
      Some((modname, symname))
    }
  }, {
    let imports = imports.clone();
    move |path| {
      imports.find(&path).map(|f| f.as_ref().clone())
    }
  });
  // Turn parsed files into a bag of rules and a list of toplevel export names
  let resolved = Rc::new(Cache::new({
    let parsed = parsed.clone();
    let exports = exports.clone();
    let imports = imports.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Module> {
      let module = Module {
        rules: parsed.find(&path)?
          .iter()
          .filter_map(|ent| {
            if let FileEntry::Rule(Rule{source, prio, target}, _) = ent {
              Some(Rule {
                source: Rc::new(
                  source.iter()
                  .map(|ex| prefix_expr(ex, &path))
                  .collect_vec()
                ),
                target: Rc::new(
                  target.iter()
                  .map(|ex| prefix_expr(ex, &path))
                  .collect_vec()
                ),
                prio: *prio,
              })
            } else { None }
          })
          .map(|Rule{ source, target, prio }| Ok(super::Rule {
            source: Rc::new(source.iter()
              .map(|ex| name_resolver.process_expression(ex))
              .collect::<Result<Vec<_>, _>>()?),
            target: Rc::new(target.iter()
              .map(|ex| name_resolver.process_expression(ex))
              .collect::<Result<Vec<_>, _>>()?),
            prio
          }))
          .collect::<ParseResult<Vec<super::Rule>>>()?,
        exports: exports.find(&path)?.clone(),
        references: imports.find(&path)?
          .values().cloned().collect()
      };
      Ok(module)
    }
  }));
  Cache::new({
    let resolved = resolved.clone();
    move |path: Rc<Vec<Spur>>, _| -> ParseResult<Vec<super::Rule>> {
      // Breadth-first search
      let mut processed: HashSet<Rc<Vec<Spur>>> = HashSet::new();
      let mut rules: Vec<super::Rule> = Vec::new();
      let mut pending: VecDeque<Rc<Vec<Spur>>> = VecDeque::new();
      pending.push_back(path);
      while let Some(el) = pending.pop_front() {
        let resolved = resolved.find(&el)?;
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
