use std::collections::HashMap;

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

pub fn load_project<F>(
    mut load_mod: F, prelude: &[&str], entry: &str
) -> Result<super::Project, ParseError>
where F: FnMut(&[&str]) -> Option<Loaded> {
    let preparser = file_parser(prelude, &[]);
    let mut loaded = Cache::new(|path: &[&str]| load_mod(path));
    let mut preparsed = Cache::new(|path: &[&str]| {
        loaded.get(path).as_ref().map(|loaded| match loaded {
            Loaded::Module(source) => Some(preparser.parse(source.as_str()).ok()?),
            _ => return None
        }).flatten()
    });
    let exports = Cache::new(|path: &[&str]| loaded.get(path).map(|data| {
        match data {
            Loaded::Namespace(names) => Some(names),
            Loaded::Module(source) => preparsed.get(path).map(|data| {
                exported_names(&data).into_iter().map(|n| n[0]).collect()
            })
        }
    }).flatten());
    let imports = Cache::new(|path: &[&str]| preparsed.get(path).map(|data| {
        data.iter().filter_map(|ent| match ent {
            FileEntry::Import(imp) => Some(imp),
            _ => None
        }).flatten().collect::<Vec<_>>()
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