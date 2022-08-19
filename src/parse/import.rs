use chumsky::{Parser, prelude::*};
use itertools::Itertools;
use mappable_rc::Mrc;
use crate::utils::iter::{box_once, box_flatten, into_boxed_iter, BoxedIterIter};
use crate::utils::{to_mrc_slice, mrc_derive};
use crate::{enum_parser, box_chain};

use super::lexer::Lexeme;

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Mrc<[String]>,
    pub name: Option<String>
}

/// initialize a BoxedIter<BoxedIter<String>> with a single element.
fn init_table(name: String) -> BoxedIterIter<'static, String> {
    // I'm not at all confident that this is a good approach.
    box_once(box_once(name))
}

/// Parse an import command
/// Syntax is same as Rust's `use` except the verb is import, no trailing semi
/// and the delimiters are plain parentheses. Namespaces should preferably contain
/// crossplatform filename-legal characters but the symbols are explicitly allowed
/// to go wild. There's a blacklist in [name]
pub fn import_parser() -> impl Parser<Lexeme, Vec<Import>, Error = Simple<Lexeme>> {
    // TODO: this algorithm isn't cache friendly, copies a lot and is generally pretty bad.
    recursive(|expr: Recursive<Lexeme, BoxedIterIter<String>, Simple<Lexeme>>| {
        enum_parser!(Lexeme::Name)
        .separated_by(just(Lexeme::NS))
        .then(
            just(Lexeme::NS)
            .ignore_then(
                choice((
                    expr.clone()
                        .separated_by(just(Lexeme::name(",")))
                        .delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
                        .map(|v| box_flatten(v.into_iter()))
                        .labelled("import group"),
                    // Each expr returns a list of imports, flatten those into a common list
                    just(Lexeme::name("*")).map(|_| init_table("*".to_string()))
                        .labelled("wildcard import"), // Just a *, wrapped
                    enum_parser!(Lexeme::Name).map(init_table)
                        .labelled("import terminal") // Just a name, wrapped
                ))
            ).or_not()
        )
        .map(|(name, opt_post): (Vec<String>, Option<BoxedIterIter<String>>)| -> BoxedIterIter<String> {
            if let Some(post) = opt_post {
                Box::new(post.map(move |el| {
                    box_chain!(name.clone().into_iter(), el)
                }))
            } else {
                box_once(into_boxed_iter(name))
            }
        })
    }).map(|paths| {
        paths.filter_map(|namespaces| {
            let path = to_mrc_slice(namespaces.collect_vec());
            let path_prefix = mrc_derive(&path, |p| &p[..p.len() - 1]);
            match path.last()?.as_str() {
                "*" => Some(Import { path: path_prefix, name: None }),
                name => Some(Import { path: path_prefix, name: Some(name.to_owned()) })
            }
        }).collect()
    }).labelled("import")
}
