use std::iter;

use chumsky::{Parser, prelude::*};
use super::name;

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub name: Option<String>
}


pub type BoxedStrIter = Box<dyn Iterator<Item = String>>;
pub type BoxedStrIterIter = Box<dyn Iterator<Item = BoxedStrIter>>;

/// initialize a Box<dyn Iterator<Item = Box<dyn Iterator<Item = String>>>>
/// with a single element.
fn init_table(name: String) -> BoxedStrIterIter {
    // I'm not confident at all that this is a good approach.
    Box::new(iter::once(Box::new(iter::once(name)) as BoxedStrIter))
}

/// Parse an import command
/// Syntax is same as Rust's `use` except the verb is import, no trailing semi
/// and the delimiters are plain parentheses. Namespaces should preferably contain
/// crossplatform filename-legal characters but the symbols are explicitly allowed
/// to go wild. There's a blacklist in [name]
pub fn import_parser() -> impl Parser<char, Vec<Import>, Error = Simple<char>> {
    // TODO: this algorithm isn't cache friendly, copies a lot and is generally pretty bad.
    recursive(|expr: Recursive<char, BoxedStrIterIter, Simple<char>>| {
        name::modname_parser()
        .padded()
        .then_ignore(just("::"))
        .repeated()
        .then(
            choice((
                expr.clone()
                .separated_by(just(','))
                .delimited_by(just('('), just(')'))
                .map(|v| Box::new(v.into_iter().flatten()) as BoxedStrIterIter),
                // Each expr returns a list of imports, flatten those into a common list
                just("*").map(|s| init_table(s.to_string())), // Just a *, wrapped
                name::modname_parser().map(init_table) // Just a name, wrapped
            )).padded()
        ).map(|(pre, post)| {
            Box::new(post.map(move |el| {
                Box::new(pre.clone().into_iter().chain(el)) as BoxedStrIter
            })) as BoxedStrIterIter
        })
    }).padded().map(|paths| {
        paths.filter_map(|namespaces| {
            let mut path: Vec<String> = namespaces.collect();
            match path.pop()?.as_str() {
                "*" => Some(Import { path, name: None }),
                name => Some(Import { path, name: Some(name.to_owned()) })
            }
        }).collect()
    })
}