use std::collections::HashSet;
use std::iter;

use crate::{enum_parser, expression::{Expr, Clause, Rule}};
use crate::utils::BoxedIter;

use super::expression::xpr_parser;
use super::import;
use super::import::import_parser;
use super::lexer::Lexeme;
use chumsky::{Parser, prelude::*};
use ordered_float::NotNan;

/// Anything we might encounter in a file
#[derive(Debug, Clone)]
pub enum FileEntry {
    Import(Vec<import::Import>),
    Comment(String),
    Rule(Rule, bool)
}

/// Recursively iterate through all "names" in an expression. It also finds a lot of things that
/// aren't names, such as all bound parameters. Generally speaking, this is not a very
/// sophisticated search.
/// 
/// TODO: find a way to exclude parameters
fn find_all_names_recur<'a>(expr: &'a Expr) -> BoxedIter<&'a Vec<String>> {
    let proc_clause = |clause: &'a Clause| match clause {
        Clause::Auto(_, typ, body) | Clause::Lambda(_, typ, body) => Box::new(
            typ.iter().flat_map(find_all_names_recur)
            .chain(body.iter().flat_map(find_all_names_recur))
        ) as BoxedIter<&'a Vec<String>>,
        Clause::S(_, body) => Box::new(
            body.iter().flat_map(find_all_names_recur)
        ),
        Clause::Name(x) => Box::new(iter::once(x)),
        _ => Box::new(iter::empty())
    };
    let Expr(val, typ) = expr;
    if let Some(t) = typ {
        Box::new(proc_clause(val).chain(find_all_names_recur(t)))
    } else { proc_clause(val) }
}

/// Collect all names that occur in an expression
fn find_all_names(expr: &Expr) -> HashSet<&Vec<String>> {
    find_all_names_recur(expr).collect()
}

fn rule_parser() -> impl Parser<Lexeme, (Vec<Expr>, NotNan<f64>, Vec<Expr>), Error = Simple<Lexeme>> {
    xpr_parser().repeated()
        .then(enum_parser!(Lexeme::Rule))
        .then(xpr_parser().repeated())
        // .map(|((lhs, prio), rhs)| )
        .map(|((a, b), c)| (a, b, c))
        .labelled("Rule")
}

pub fn line_parser() -> impl Parser<Lexeme, FileEntry, Error = Simple<Lexeme>> {
    choice((
        // In case the usercode wants to parse doc
        enum_parser!(Lexeme >> FileEntry; Comment),
        just(Lexeme::name("import"))
            .ignore_then(import_parser().map(FileEntry::Import))
            .then_ignore(enum_parser!(Lexeme::Comment)),
        just(Lexeme::name("export")).map_err_with_span(|e, s| {
            println!("{:?} could not yield an export", s); e
        })
            .ignore_then(rule_parser())
            .map(|(source, prio, target)| FileEntry::Rule(Rule{source, prio, target}, true)),
        // This could match almost anything so it has to go last
        rule_parser().map(|(source, prio, target)| FileEntry::Rule(Rule{source, prio, target}, false)),
    ))
}

/// Collect all exported names (and a lot of other words) from a file
pub fn exported_names(src: &Vec<FileEntry>) -> HashSet<&Vec<String>> {
    src.iter().flat_map(|ent| match ent {
        FileEntry::Rule(Rule{source, target, ..}, true) =>
            Box::new(source.iter().chain(target.iter())) as BoxedIter<&Expr>,
        _ => Box::new(iter::empty())
    }).map(find_all_names).flatten().collect()
}

/// Summarize all imports from a file in a single list of qualified names 
pub fn imports<'a, 'b, I>(
    src: I
) -> impl Iterator<Item = &'b import::Import> + 'a
where I: Iterator<Item = &'b FileEntry> + 'a {
    src.filter_map(|ent| match ent {
        FileEntry::Import(impv) => Some(impv.iter()),
        _ => None
    }).flatten()
}