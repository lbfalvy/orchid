use std::collections::HashSet;
use std::iter;

use crate::enum_parser;
use crate::expression::{Expr, Clause, Rule};
use crate::utils::BoxedIter;
use crate::utils::Stackframe;

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

fn visit_all_names_clause_recur<'a, F>(
    clause: &'a Clause,
    binds: Stackframe<String>,
    mut cb: &mut F
) where F: FnMut(&'a Vec<String>) {
    match clause {
        Clause::Auto(name, typ, body) => {
            for x in typ.iter() {
                visit_all_names_expr_recur(x, binds.clone(), &mut cb)
            }
            let binds_dup = binds.clone();
            let new_binds = if let Some(n) = name {
                binds_dup.push(n.to_owned())
            } else {
                binds
            };
            for x in body.iter() {
                visit_all_names_expr_recur(x, new_binds.clone(), &mut cb)
            }
        },
        Clause::Lambda(name, typ, body) => {
            for x in typ.iter() {
                visit_all_names_expr_recur(x, binds.clone(), &mut cb)
            }
            for x in body.iter() {
                visit_all_names_expr_recur(x, binds.push(name.to_owned()), &mut cb)
            }
        },
        Clause::S(_, body) => for x in body.iter() {
            visit_all_names_expr_recur(x, binds.clone(), &mut cb)
        },
        Clause::Name{ local, qualified } => {
            if let Some(name) = local {
                if binds.iter().all(|x| x != name) {
                    cb(qualified)
                }
            }
        }
        _ => (),
    }
}

/// Recursively iterate through all "names" in an expression. It also finds a lot of things that
/// aren't names, such as all bound parameters. Generally speaking, this is not a very
/// sophisticated search.
/// 
/// TODO: find a way to exclude parameters
fn visit_all_names_expr_recur<'a, F>(
    expr: &'a Expr,
    binds: Stackframe<String>,
    cb: &mut F
) where F: FnMut(&'a Vec<String>) {
    let Expr(val, typ) = expr;
    visit_all_names_clause_recur(val, binds.clone(), cb);
    if let Some(t) = typ {
        visit_all_names_expr_recur(t, binds, cb)
    }
}

/// Collect all names that occur in an expression
fn find_all_names(expr: &Expr) -> HashSet<&Vec<String>> {
    let mut ret = HashSet::new();
    visit_all_names_expr_recur(expr, Stackframe::new(String::new()), &mut |n| {
        if !n.last().unwrap().starts_with("$") {
            ret.insert(n);
        }
    });
    ret
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