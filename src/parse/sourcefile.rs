use std::collections::HashSet;

use crate::{enum_parser, box_chain};
use crate::expression::{Expr, Clause, Rule};
use crate::utils::to_mrc_slice;
use crate::utils::Stackframe;
use crate::utils::iter::box_empty;

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
    Rule(Rule, bool),
    Export(Vec<Vec<String>>)
}

fn visit_all_names_clause_recur<'a, F>(
    clause: &'a Clause,
    binds: Stackframe<String>,
    cb: &mut F
) where F: FnMut(&'a [String]) {
    match clause {
        Clause::Auto(name, typ, body) => {
            for x in typ.iter() {
                visit_all_names_expr_recur(x, binds.clone(), cb)
            }
            let binds_dup = binds.clone();
            let new_binds = if let Some(n) = name {
                binds_dup.push(n.to_owned())
            } else {
                binds
            };
            for x in body.iter() {
                visit_all_names_expr_recur(x, new_binds.clone(), cb)
            }
        },
        Clause::Lambda(name, typ, body) => {
            for x in typ.iter() {
                visit_all_names_expr_recur(x, binds.clone(), cb)
            }
            for x in body.iter() {
                visit_all_names_expr_recur(x, binds.push(name.to_owned()), cb)
            }
        },
        Clause::S(_, body) => for x in body.iter() {
            visit_all_names_expr_recur(x, binds.clone(), cb)
        },
        Clause::Name{ local: Some(name), qualified } => {
            if binds.iter().all(|x| x != name) {
                cb(qualified)
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
) where F: FnMut(&'a [String]) {
    let Expr(val, typ) = expr;
    visit_all_names_clause_recur(val, binds.clone(), cb);
    if let Some(t) = typ {
        visit_all_names_expr_recur(t, binds, cb)
    }
}

/// Collect all names that occur in an expression
fn find_all_names(expr: &Expr) -> HashSet<&[String]> {
    let mut ret = HashSet::new();
    visit_all_names_expr_recur(expr, Stackframe::new(String::new()), &mut |n| {
        if !n.last().unwrap().starts_with('$') {
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
        }).ignore_then(
            just(Lexeme::NS).ignore_then(
                enum_parser!(Lexeme::Name).map(|n| vec![n])
                .separated_by(just(Lexeme::name(",")))
                .delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
            ).map(FileEntry::Export)
        ).or(rule_parser().map(|(source, prio, target)| {
            FileEntry::Rule(Rule {
                source: to_mrc_slice(source),
                prio,
                target: to_mrc_slice(target)
            }, true)
        })),
        // This could match almost anything so it has to go last
        rule_parser().map(|(source, prio, target)| FileEntry::Rule(Rule{
            source: to_mrc_slice(source),
            prio,
            target: to_mrc_slice(target)
        }, false)),
    ))
}

/// Collect all exported names (and a lot of other words) from a file
pub fn exported_names(src: &[FileEntry]) -> HashSet<&[String]> {
    src.iter().flat_map(|ent| match ent {
        FileEntry::Rule(Rule{source, target, ..}, true) =>
            box_chain!(source.iter(), target.iter()),
        _ => box_empty()
    }).flat_map(find_all_names).chain(
        src.iter().filter_map(|ent| {
            if let FileEntry::Export(names) = ent {Some(names.iter())} else {None}
        }).flatten().map(Vec::as_slice)
    ).collect()
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
