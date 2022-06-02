use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::{iter, clone};

use chumsky::{Parser, prelude::Simple};
use thiserror::Error;

use crate::parse::{self, file_parser, exported_names, FileEntry};
use crate::utils::Cache;

#[derive(Debug, Clone)]
pub enum Loaded {
    Module(String),
    Namespace(Vec<String>)
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Failed to parse {file}: {errors:?}")]
    Syntax {
        file: String,
        errors: Vec<Simple<char>>
    },
    #[error("Expected {0}, found {1}")]
    Mismatch(String, String),
    
}

impl ParseError {
    pub fn not_found(name: &str) -> ParseError { ParseError::NotFound(name.to_string()) }
    pub fn syntax(file: &str, errors: Vec<Simple<char>>) -> ParseError {
        ParseError::Syntax { file: file.to_string(), errors }
    }
    pub fn mismatch(expected: &str, found: &str) -> ParseError {
        ParseError::Mismatch(expected.to_string(), found.to_string())
    }
}

// Loading a module:
//  1. [X] Parse the imports
//  2. [ ] Build a mapping of all imported symbols to full paths
//     -> [X] Parse the exported symbols from all imported modules
//  3. [ ] Parse everything using the full list of operators
//  4. [ ] Traverse and remap elements

type GetLoaded<'a> = dyn FnMut(&'a [&str]) -> &'a Option<Loaded>;
type GetPreparsed<'a> = dyn FnMut(&'a [&str]) -> &'a Option<Vec<FileEntry>>;

pub fn load_project<'a, F>(
    mut load_mod: F, prelude: &[&'a str], entry: &str
) -> Result<super::Project, ParseError>
where F: FnMut(&[&str]) -> Option<Loaded> {
    // TODO: Welcome to Kamino!
    let prelude_vec: Vec<String> = prelude.iter().map(|s| s.to_string()).collect();
    let preparser = file_parser(prelude, &[]);
    let loaded_cell = RefCell::new(Cache::new(|path: Vec<String>| {
        load_mod(&path.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }));
    let preparsed_cell = RefCell::new(Cache::new(|path: Vec<String>| {
        let mut loaded = loaded_cell.borrow_mut();
        loaded.by_clone(path).as_ref().map(|loaded| match loaded {
            Loaded::Module(source) => Some(preparser.parse(source.as_str()).ok()?),
            _ => return None
        }).flatten()
    }));
    let exports_cell = RefCell::new(Cache::new(|path: Vec<String>| {
        let mut loaded = loaded_cell.borrow_mut();
        loaded.by_clone(path.clone()).as_ref().map(|data| {
            let mut preparsed = preparsed_cell.borrow_mut();
            match data {
                Loaded::Namespace(names) => Some(names.clone()),
                Loaded::Module(source) => preparsed.by_clone(path).as_ref().map(|data| {
                    parse::exported_names(&data).into_iter()
                        .map(|n| n[0].clone())
                        .collect()
                }),
                _ => None
            }
        }).flatten()
    }));
    let imports_cell = RefCell::new(Cache::new(|path: Vec<String>| {
        let mut preparsed = preparsed_cell.borrow_mut();
        let entv = preparsed.by_clone(path).clone()?;
        let import_entries = parse::imports(entv.iter());
        let mut imported_symbols: HashMap<String, Vec<String>> = HashMap::new();
        for imp in import_entries {
            let mut exports = exports_cell.borrow_mut();
            let export = exports.by_clone(imp.path.clone()).as_ref()?;
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
        Some(imported_symbols)
    }));
    let parsed = RefCell::new(Cache::new(|path: Vec<String>| {
        let mut imports = imports_cell.borrow_mut();
        let mut loaded = loaded_cell.borrow_mut();
        let data = loaded.by_clone(path.clone()).as_ref()?;
        let text = match data { Loaded::Module(s) => Some(s), _ => None }?;
        let imported_symbols = imports.by_clone(path).as_ref()?;
        let imported_ops: Vec<&str> = imported_symbols.keys()
            .chain(prelude_vec.iter())
            .map(|s| s.as_str())
            .filter(|s| parse::is_op(s))
            .collect();
        let file_parser = file_parser(prelude, &imported_ops);
        file_parser.parse(text.as_str()).ok()
    }));
    // let main = preparsed.get(&[entry]);
    // for imp in parse::imports(main) {
    //     if !modules.contains_key(&imp.path) {
    //         if modules[&imp.path] 
    //     }
    // }
    // let mut project = super::Project {
    //     modules: HashMap::new()
    // };
    
    // Some(project)
    todo!("Finish this function")
}