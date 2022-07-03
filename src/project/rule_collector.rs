// use std::collections::{HashMap, HashSet, VecDeque};
// use std::fmt::Debug;
// use std::rc::Rc;

// use chumsky::Parser;

// use crate::parse::{self, line_parser, FileEntry};
// use crate::utils::Cache;


// use super::name_resolver::NameResolver;
// use super::parse_error::ParseError;
// use super::prefix::prefix_expr;
// use super::loaded::Loaded;

// type ParseResult<T, ELoad> = Result<T, ParseError<ELoad>>; 

// pub fn rule_collector<F: 'static, ELoad>(
//     mut load_mod: F,
//     prelude: Vec<String>
// // ) -> impl FnMut(Vec<String>) -> Result<&'a Vec<super::Rule>, ParseError<ELoad>> + 'a
// ) -> Cache<Vec<String>, Result<Vec<super::Rule>, ParseError<ELoad>>>
// where
//     F: FnMut(Vec<String>) -> Result<Loaded, ELoad>,
//     ELoad: Clone + Debug
// {
//     // Map paths to a namespace with name list (folder) or module with source text (file)
//     let loaded = Rc::new(Cache::new(move |path: Vec<String>|
//          -> ParseResult<Loaded, ELoad> {
//         load_mod(path).map_err(ParseError::Load)
//     }));
//     // Map names to the longest prefix that points to a valid module
//     let modname = Rc::new(Cache::new({
//         let loaded = Rc::clone(&loaded);
//         move |symbol: Vec<String>| -> Result<Vec<String>, Vec<ParseError<ELoad>>> {
//             let mut errv: Vec<ParseError<ELoad>> = Vec::new();
//             let reg_err = |e, errv: &mut Vec<ParseError<ELoad>>| {
//                 errv.push(e);
//                 if symbol.len() == errv.len() { Err(errv.clone()) }
//                 else { Ok(()) }
//             };
//             loop {
//                 let (path, _) = symbol.split_at(symbol.len() - errv.len());
//                 let pathv = path.to_vec();
//                 match loaded.try_find(&pathv) {
//                     Ok(imports) => match imports.as_ref() {
//                         Loaded::Module(_) => break Ok(pathv.clone()),
//                         _ => reg_err(ParseError::None, &mut errv)?
//                     },
//                     Err(err) => reg_err(err, &mut errv)?
//                 }
//             }
//         }
//     }));
//     // Preliminarily parse a file, substitution rules and imports are valid
//     let preparsed = Rc::new(Cache::new({
//         let preparser = line_parser(&prelude, &prelude);
//         let loaded = Rc::clone(&loaded);
//         move |path: Vec<String>| -> ParseResult<Vec<FileEntry>, ELoad> {
//             let loaded = loaded.try_find(&path)?;
//             if let Loaded::Module(source) = loaded.as_ref() {
//                 Ok(preparser.parse(source.as_str())?)
//             } else {Err(ParseError::None)}
//         }
//     }));
//     // Collect all toplevel names exported from a given file
//     let exports = Rc::new(Cache::new({
//         let loaded = Rc::clone(&loaded);
//         let preparsed = Rc::clone(&preparsed);
//         move |path: Vec<String>| -> ParseResult<Vec<String>, ELoad> {
//             let loaded = loaded.try_find(&path)?;
//             if let Loaded::Namespace(names) = loaded.as_ref() {
//                 return Ok(names.clone());
//             }
//             let preparsed = preparsed.try_find(&path)?;
//             Ok(parse::exported_names(&preparsed)
//                 .into_iter()
//                 .map(|n| n[0].clone())
//                 .collect())
//         }
//     }));
//     // Collect all toplevel names imported by a given file
//     let imports = Rc::new(Cache::new({
//         let preparsed = Rc::clone(&preparsed);
//         let exports = Rc::clone(&exports);
//         move |path: Vec<String>| -> ParseResult<HashMap<String, Vec<String>>, ELoad> {
//             let entv = preparsed.try_find(&path)?.clone();
//             let import_entries = parse::imports(entv.iter());
//             let mut imported_symbols: HashMap<String, Vec<String>> = HashMap::new();
//             for imp in import_entries {
//                 let export = exports.try_find(&imp.path)?;
//                 if let Some(ref name) = imp.name {
//                     if export.contains(&name) {
//                         imported_symbols.insert(name.clone(), imp.path.clone());
//                     }
//                 } else {
//                     for exp in export.as_ref() {
//                         imported_symbols.insert(exp.clone(), imp.path.clone());
//                     }
//                 }
//             }
//             Ok(imported_symbols)
//         }
//     }));
//     // Final parse, operators are correctly separated
//     let parsed = Rc::new(Cache::new({
//         let imports = Rc::clone(&imports);
//         let loaded = Rc::clone(&loaded);
//         move |path: Vec<String>| -> ParseResult<Vec<FileEntry>, ELoad> {
//             let imported_ops: Vec<String> =
//                 imports.try_find(&path)?
//                 .keys()
//                 .chain(prelude.iter())
//                 .filter(|s| parse::is_op(s))
//                 .cloned()
//                 .collect();
//             let parser = file_parser(&prelude, &imported_ops);
//             if let Loaded::Module(source) = loaded.try_find(&path)?.as_ref() {
//                 Ok(parser.parse(source.as_str())?)
//             } else { Err(ParseError::None) }
//         }
//     }));
//     let mut name_resolver = NameResolver::new({
//         let modname = Rc::clone(&modname);
//         move |path| {
//             Some(modname.try_find(path).ok()?.as_ref().clone())
//         }
//     }, {
//         let imports = Rc::clone(&imports);
//         move |path| {
//             imports.try_find(path).map(|f| f.as_ref().clone())
//         }
//     });
//     // Turn parsed files into a bag of rules and a list of toplevel export names
//     let resolved = Rc::new(Cache::new({
//         let parsed = Rc::clone(&parsed);
//         let exports = Rc::clone(&exports);
//         let imports = Rc::clone(&imports);
//         let modname = Rc::clone(&modname);
//         move |path: Vec<String>| -> ParseResult<super::Module, ELoad> {
//             let module = super::Module {
//                 rules: parsed.try_find(&path)?
//                     .iter()
//                     .filter_map(|ent| {
//                         if let FileEntry::Export(s) | FileEntry::Rule(s) = ent {
//                             Some(super::Rule {
//                                 source: prefix_expr(&s.source, &path),
//                                 target: prefix_expr(&s.target, &path),
//                                 priority: s.priority,
//                             })
//                         } else { None }
//                     })
//                     .map(|rule| Ok(super::Rule {
//                         source: name_resolver.process_expression(&rule.source)?,
//                         target: name_resolver.process_expression(&rule.target)?,
//                         ..rule
//                     }))
//                     .collect::<ParseResult<Vec<super::Rule>, ELoad>>()?,
//                 exports: exports.try_find(&path)?.as_ref().clone(),
//                 references: imports.try_find(&path)?
//                     .values()
//                     .filter_map(|imps| {
//                         modname.try_find(&imps).ok().map(|r| r.as_ref().clone())
//                     })
//                     .collect()
//             };
//             Ok(module)
//         }
//     }));
//     let all_rules = Cache::new({
//         let resolved = Rc::clone(&resolved);
//         move |path: Vec<String>| -> ParseResult<Vec<super::Rule>, ELoad> {
//             let mut processed: HashSet<Vec<String>> = HashSet::new();
//             let mut rules: Vec<super::Rule> = Vec::new();
//             let mut pending: VecDeque<Vec<String>> = VecDeque::new();
//             pending.push_back(path);
//             while let Some(el) = pending.pop_front() {
//                 let resolved = resolved.try_find(&el)?;
//                 processed.insert(el.clone());
//                 pending.extend(
//                     resolved.references.iter()
//                         .filter(|&v| !processed.contains(v))
//                         .cloned() 
//                 );
//                 rules.extend(
//                     resolved.rules.iter().cloned()
//                 )
//             };
//             Ok(rules)
//         }
//     });
//     return all_rules; 
// }
