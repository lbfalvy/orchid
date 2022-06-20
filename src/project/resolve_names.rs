use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error;

use chumsky::{prelude::Simple, Parser};
use thiserror::Error;

use crate::parse::{self, file_parser, FileEntry};
use crate::utils::{Cache, as_modpath};

use super::expr;
use super::name_resolver::{NameResolver, ResolutionError};
use super::prefix::prefix;

#[derive(Debug, Clone)]
pub enum Loaded {
    Module(String),
    Namespace(Vec<String>),
}

#[derive(Error, Debug, Clone)]
pub enum ParseError<ELoad> where ELoad: Clone {
    #[error("Resolution cycle")]
    ResolutionCycle,
    #[error("File not found: {0}")]
    Load(ELoad),
    #[error("Failed to parse: {0:?}")]
    Syntax(Vec<Simple<char>>),
    #[error("Not a module")]
    None
}

impl<T> From<Vec<Simple<char>>> for ParseError<T> where T: Clone {
    fn from(simp: Vec<Simple<char>>) -> Self { Self::Syntax(simp) }
}

impl<T> From<ResolutionError<ParseError<T>>> for ParseError<T> where T: Clone {
    fn from(res: ResolutionError<ParseError<T>>) -> Self {
        match res {
            ResolutionError::Cycle(_) => ParseError::ResolutionCycle,
            ResolutionError::NoModule(_) => ParseError::None,
            ResolutionError::Delegate(d) => d
        }
    }
}

type ImportMap = HashMap<String, Vec<String>>;
type ParseResult<T, ELoad> = Result<T, ParseError<ELoad>>; 
type AnyParseResult<T, ELoad> = Result<T, Vec<ParseError<ELoad>>>;

pub fn load_project<'a, F, ELoad>(
    mut load_mod: F,
    prelude: &[&'a str],
    entry: (Vec<String>, expr::Expr),
) -> Result<super::Project, ParseError<ELoad>>
where
    F: FnMut(&[&str]) -> Result<Loaded, ELoad>,
    ELoad: Clone
{
    let prelude_vec: Vec<String> = prelude.iter().map(|s| s.to_string()).collect();
    let preparser = file_parser(prelude, &[]);
    // Map paths to a namespace with name list (folder) or module with source text (file)
    let loaded_cell = RefCell::new(Cache::new(|path: Vec<String>|
         -> ParseResult<Loaded, ELoad> {
        load_mod(&path.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .map_err(ParseError::Load)
    }));
    let modname_cell = RefCell::new(Cache::new(|symbol: Vec<String>|
        -> AnyParseResult<Vec<String>, ELoad> {
        let mut local_loaded = loaded_cell.borrow_mut();
        let mut errv: Vec<ParseError<ELoad>> = Vec::new();
        loop {
            let (path, name) = symbol.split_at(symbol.len() - errv.len());
            let pathv = path.to_vec();
            match local_loaded.by_clone_fallible(&pathv) {
                Ok(imports) => break Ok(pathv.clone()),
                Err(err) => {
                    errv.push(err);
                    if symbol.len() == errv.len() {
                        break Err(errv);
                    }
                }
            }
        }
    }));
    // Preliminarily parse a file, substitution patterns and imports are valid
    let preparsed_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<Vec<FileEntry>, ELoad> {
        let mut loaded = loaded_cell.borrow_mut();
        let loaded = loaded.by_clone_fallible(&path)?;
        if let Loaded::Module(source) = loaded {
            Ok(preparser.parse(source.as_str())?)
        } else {Err(ParseError::None)}
    }));
    // Collect all toplevel names exported from a given file
    let exports_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<Vec<String>, ELoad> {
        let mut local_loaded = loaded_cell.borrow_mut();
        let loaded = local_loaded.by_clone_fallible(&path)?;
        let mut local_preparsed = preparsed_cell.borrow_mut();
        if let Loaded::Namespace(names) = loaded {
            return Ok(names.clone());
        }
        let preparsed = local_preparsed.by_clone_fallible(&path)?;
        Ok(parse::exported_names(&preparsed)
            .into_iter()
            .map(|n| n[0].clone())
            .collect())
    }));
    // Collect all toplevel names imported by a given file
    let imports_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<ImportMap, ELoad> {
        let mut local_preparsed = preparsed_cell.borrow_mut();
        let entv = local_preparsed.by_clone_fallible(&path)?.clone();
        let import_entries = parse::imports(entv.iter());
        let mut imported_symbols: HashMap<String, Vec<String>> = HashMap::new();
        for imp in import_entries {
            let mut exports = exports_cell.borrow_mut();
            let export = exports.by_clone_fallible(&imp.path)?;
            if let Some(ref name) = imp.name {
                if export.contains(&name) {
                    imported_symbols.insert(name.clone(), imp.path.clone());
                }
            } else {
                for exp in export.clone() {
                    imported_symbols.insert(exp.clone(), imp.path.clone());
                }
            }
        }
        Ok(imported_symbols)
    }));
    // Final parse, operators are correctly separated
    let parsed_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<Vec<FileEntry>, ELoad> {
        let mut local_imports = imports_cell.borrow_mut();
        let imports = local_imports.by_clone_fallible(&path)?;
        let mut local_loaded = loaded_cell.borrow_mut();
        let imported_ops: Vec<&str> = imports
            .keys()
            .chain(prelude_vec.iter())
            .map(|s| s.as_str())
            .filter(|s| parse::is_op(s))
            .collect();
        let parser = file_parser(prelude, &imported_ops);
        if let Loaded::Module(source) = local_loaded.by_clone_fallible(&path)? {
            Ok(parser.parse(source.as_str())?)
        } else {Err(ParseError::None)}
    }));
    let mut name_resolver = NameResolver::new(
        |path: &Vec<String>| { modname_cell.borrow_mut().by_clone_fallible(path).cloned().ok() },
        |path: &Vec<String>| { imports_cell.borrow_mut().by_clone_fallible(path).cloned() }
    );
    // Turn parsed files into a bag of substitutions and a list of toplevel export names
    let resolved_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<super::Module, ELoad> {
        let mut parsed = parsed_cell.borrow_mut();
        let parsed_entries = parsed.by_clone_fallible(&path)?;
        let subs: Vec<super::Substitution> = parsed_entries
            .iter()
            .filter_map(|ent| {
                if let FileEntry::Export(s) | FileEntry::Substitution(s) = ent {
                    Some(super::Substitution {
                        source: prefix(&s.source, &path),
                        target: prefix(&s.target, &path),
                        priority: s.priority,
                    })
                } else { None }
            })
            .map(|sub| Ok(super::Substitution {
                source: name_resolver.process_expression(&sub.source)?,
                target: name_resolver.process_expression(&sub.target)?,
                ..sub
            }))
            .collect::<ParseResult<Vec<super::Substitution>, ELoad>>()?;
        let module = super::Module {
            substitutions: subs,
            exports: exports_cell
                .borrow_mut()
                .by_clone_fallible(&path)?
                .clone(),
            references: imports_cell
                .borrow_mut()
                .by_clone_fallible(&path)?
                .values()
                .filter_map(|imps| modname_cell.borrow_mut().by_clone_fallible(imps).ok().cloned())
                .collect()
        };
        Ok(module)
    }));
    let all_subs_cell = RefCell::new(Cache::new(|path: Vec<String>|
        -> ParseResult<Vec<super::Substitution>, ELoad> {
        let mut processed: HashSet<Vec<String>> = HashSet::new();
        let mut subs: Vec<super::Substitution> = Vec::new();
        let mut pending: VecDeque<Vec<String>> = VecDeque::new();
        while let Some(el) = pending.pop_front() {
            let mut local_resolved = resolved_cell.borrow_mut();
            let resolved = local_resolved.by_clone_fallible(&el)?;
            processed.insert(el.clone());
            pending.extend(
                resolved.references.iter()
                    .filter(|&v| !processed.contains(v))
                    .cloned()
            );
            subs.extend(
                resolved.substitutions.iter().cloned()
            )
        };
        Ok(subs)
    }));
    // let substitutions =
    // let main = preparsed.get(&[entry]);
    // for imp in parse::imports(main) {
    //     if !modules.contains_key(&imp.path) {
    //         if modules[&imp.path]
    //     }
    // }
    // let mut project = super::Project {
    //     modules: HashMap::new()
    // };
    todo!("Finish this function")
}
