use chumsky::{Parser, prelude::*, text::Character};
use super::name;

enum Import {
    Name(Vec<String>, String),
    All(Vec<String>)
}

fn prefix(pre: Vec<String>, im: Import) -> Import {
    match im {
        Import::Name(ns, name) => Import::Name(
            pre.into_iter().chain(ns.into_iter()).collect(),
            name
        ),
        Import::All(ns) => Import::All(
            pre.into_iter().chain(ns.into_iter()).collect()
        )
    }
}


type BoxedStrIter = Box<dyn Iterator<Item = String>>;
type BoxedStrIterIter = Box<dyn Iterator<Item = BoxedStrIter>>;

fn init_table(name: String) -> BoxedStrIterIter {
    Box::new(vec![Box::new(vec![name].into_iter()) as BoxedStrIter].into_iter())
}

pub fn import_parser() -> impl Parser<char, Vec<Import>, Error = Simple<char>> {
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
                    just("*").map(|s| init_table(s.to_string())),
                    name::modname_parser().map(init_table)
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
                "*" => Some(Import::All(path)),
                name => Some(Import::Name(path, name.to_owned()))
            }
        }).collect()
    })
}