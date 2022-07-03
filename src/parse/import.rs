use std::iter;

use chumsky::{Parser, prelude::*};
use crate::{enum_parser, utils::BoxedIter};

use super::lexer::Lexeme;

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub name: Option<String>
}

/// initialize a BoxedIter<BoxedIter<String>> with a single element.
fn init_table(name: String) -> BoxedIter<'static, BoxedIter<'static, String>> {
    // I'm not at all confident that this is a good approach.
    Box::new(iter::once(Box::new(iter::once(name)) as BoxedIter<String>))
}

/// Parse an import command
/// Syntax is same as Rust's `use` except the verb is import, no trailing semi
/// and the delimiters are plain parentheses. Namespaces should preferably contain
/// crossplatform filename-legal characters but the symbols are explicitly allowed
/// to go wild. There's a blacklist in [name]
pub fn import_parser() -> impl Parser<Lexeme, Vec<Import>, Error = Simple<Lexeme>> {
    // TODO: this algorithm isn't cache friendly, copies a lot and is generally pretty bad.
    recursive(|expr: Recursive<Lexeme, BoxedIter<BoxedIter<String>>, Simple<Lexeme>>| {
        enum_parser!(Lexeme::Name)
        .separated_by(just(Lexeme::NS))
        .then(
            just(Lexeme::NS)
            .ignore_then(
                choice((
                    expr.clone()
                        .separated_by(just(Lexeme::name(",")))
                        .delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
                        .map(|v| Box::new(v.into_iter().flatten()) as BoxedIter<BoxedIter<String>>)
                        .labelled("import group"),
                    // Each expr returns a list of imports, flatten those into a common list
                    just(Lexeme::name("*")).map(|_| init_table("*".to_string()))
                        .labelled("wildcard import"), // Just a *, wrapped
                    enum_parser!(Lexeme::Name).map(init_table)
                        .labelled("import terminal") // Just a name, wrapped
                ))
            ).or_not()
        )
        .map(|(name, opt_post): (Vec<String>, Option<BoxedIter<BoxedIter<String>>>)| -> BoxedIter<BoxedIter<String>> {
            if let Some(post) = opt_post {
                Box::new(post.map(move |el| {
                    Box::new(name.clone().into_iter().chain(el)) as BoxedIter<String>
                })) as BoxedIter<BoxedIter<String>>
            } else {
                Box::new(iter::once(Box::new(name.into_iter()) as BoxedIter<String>))
            }
        })
    }).map(|paths| {
        paths.filter_map(|namespaces| {
            let mut path: Vec<String> = namespaces.collect();
            match path.pop()?.as_str() {
                "*" => Some(Import { path, name: None }),
                name => Some(Import { path, name: Some(name.to_owned()) })
            }
        }).collect()
    }).labelled("import")
}