use std::collections::HashSet;
use std::iter;

use super::expression::Expr;
use super::import;
use super::misc;
use super::substitution::substitution_parser;
use super::substitution::Substitution;
use chumsky::{Parser, prelude::*};

/// Anything we might encounter in a file
#[derive(Debug, Clone)]
pub enum FileEntry {
    Import(Vec<import::Import>),
    Comment(String),
    Substitution(Substitution),
    Export(Substitution)
}

/// Recursively iterate through all "names" in an expression. It also finds a lot of things that
/// aren't names, such as all bound parameters. Generally speaking, this is not a very
/// sophisticated search.
/// 
/// TODO: find a way to exclude parameters
fn find_all_names_recur(expr: &Expr) -> Box<dyn Iterator<Item = &Vec<String>> + '_> {
    match expr {
        Expr::Auto(_, typ, body) | Expr::Lambda(_, typ, body) => Box::new(match typ {
            Some(texp) => find_all_names_recur(texp),
            None => Box::new(iter::empty())
        }.chain(body.into_iter().map(find_all_names_recur).flatten())),
        Expr::S(body) => Box::new(body.into_iter().map(find_all_names_recur).flatten()),
        Expr::Typed(val, typ) => Box::new(
            find_all_names_recur(val).chain(find_all_names_recur(typ))
        ),
        Expr::Name(x) => Box::new(iter::once(x)),
        _ => Box::new(iter::empty())
    }
}

/// Collect all names that occur in an expression
fn find_all_names(expr: &Expr) -> HashSet<&Vec<String>> {
    find_all_names_recur(expr).collect()
}

/// Parse a file into a list of distinctive entries
pub fn file_parser<'a>(
    pattern_ops: &[&'a str], ops: &[&'a str]
) -> impl Parser<char, Vec<FileEntry>, Error = Simple<char>> + 'a {
    choice((
        // In case the usercode wants to parse doc
        misc::comment_parser().map(FileEntry::Comment),
        import::import_parser().map(FileEntry::Import),
        text::keyword("export")
            .ignore_then(substitution_parser(pattern_ops, ops)).map(FileEntry::Export),
        // This could match almost anything so it has to go last
        substitution_parser(pattern_ops, ops).map(FileEntry::Substitution)
    )).padded()
    .separated_by(just('\n'))
    .then_ignore(end())
}

/// Decide if a string can be an operator. Operators can include digits and text, just not at the
/// start.
pub fn is_op(s: &str) -> bool {
    return match s.chars().next() {
        Some(x) => !x.is_alphanumeric(), 
        None => false
    }
}

/// Collect all exported names (and a lot of other words) from a file
pub fn exported_names(src: &Vec<FileEntry>) -> HashSet<&Vec<String>> {
    src.iter().filter_map(|ent| match ent {
        FileEntry::Export(a) => Some(&a.source),
        _ => None
    }).map(find_all_names).flatten().collect()
}

/// Collect all operators defined in a file (and some other words)
fn defined_ops(src: &Vec<FileEntry>, exported_only: bool) -> Vec<&String> {
    let all_names:HashSet<&Vec<String>> = src.iter().filter_map(|ent| match ent {
        FileEntry::Substitution(a) => if exported_only {None} else {Some(&a.source)},
        FileEntry::Export(a) => Some(&a.source),
        _ => None
    }).map(find_all_names).flatten().collect();
    // Dedupe stage of dubious value; collecting into a hashset may take longer than
    // handling duplicates would with a file of sensible size.
    all_names.into_iter()
        .filter_map(|name|
            // If it's namespaced, it's imported.
            if name.len() == 1 && is_op(&name[0]) {Some(&name[0])}
            else {None}
        ).collect()
}

/// Collect all operators from a file
pub fn all_ops(src: &Vec<FileEntry>) -> Vec<&String> { defined_ops(src, false) }
/// Collect exported operators from a file (plus some extra)
pub fn exported_ops(src: &Vec<FileEntry>) -> Vec<&String> { defined_ops(src, true) }

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