use std::{ops::Range, iter, fmt};
use ordered_float::NotNan;
use chumsky::{Parser, prelude::*};
use std::fmt::Debug;
use crate::utils::BoxedIter;

use super::{number, string, name, comment};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Entry(pub Lexeme, pub Range<usize>);
impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
        // f.debug_tuple("Entry").field(&self.0).field(&self.1).finish()
    }
}
impl Into<(Lexeme, Range<usize>)> for Entry {
    fn into(self) -> (Lexeme, Range<usize>) {
        (self.0, self.1)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Lexeme {
    Num(NotNan<f64>),
    Int(u64),
    Char(char),
    Str(String),
    Name(String),
    Rule(NotNan<f64>),
    NS, // namespace separator
    LP(char),
    RP(char),
    BS, // Backslash
    At,
    Type, // type operator
    Comment(String)
}

impl Debug for Lexeme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(n) => write!(f, "{}", n),
            Self::Int(i) => write!(f, "{}", i),
            Self::Char(c) => write!(f, "{:?}", c),
            Self::Str(s) => write!(f, "{:?}", s),
            Self::Name(name) => write!(f, "{}", name),
            Self::Rule(prio) => write!(f, "={}=>", prio),
            Self::NS => write!(f, "::"),
            Self::LP(l) => write!(f, "{}", l),
            Self::RP(l) => match l {
                '(' => write!(f, ")"),
                '[' => write!(f, "]"),
                '{' => write!(f, "}}"),
                _ => f.debug_tuple("RP").field(l).finish()
            },
            Self::BS => write!(f, "\\"),
            Self::At => write!(f, "@"),
            Self::Type => write!(f, ":"),
            Self::Comment(text) => write!(f, "--[{}]--", text),
        }
    }
}

impl Lexeme {
    pub fn name<T: ToString>(n: T) -> Self {
        Lexeme::Name(n.to_string())
    }
    pub fn rule<T>(prio: T) -> Self where T: Into<f64> {
        Lexeme::Rule(NotNan::new(prio.into()).expect("Rule priority cannot be NaN"))
    }
    pub fn paren_parser<T, P>(
        expr: P
    ) -> impl Parser<Lexeme, (char, T), Error = Simple<Lexeme>> + Clone
    where P: Parser<Lexeme, T, Error = Simple<Lexeme>> + Clone {
        choice((
            expr.clone().delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
                .map(|t| ('(', t)),
            expr.clone().delimited_by(just(Lexeme::LP('[')), just(Lexeme::RP('[')))
                .map(|t| ('[', t)),
            expr.delimited_by(just(Lexeme::LP('{')), just(Lexeme::RP('{')))
                .map(|t| ('{', t)),
        ))
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LexedText(pub Vec<Vec<Entry>>);

impl Debug for LexedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in &self.0 {
            for tok in row {
                tok.fmt(f)?;
                f.write_str(" ")?
            }
            f.write_str("\n")?
        }
        Ok(())
    }
}

type LexSubres<'a> = BoxedIter<'a, Entry>;

fn paren_parser<'a>(
    expr: Recursive<'a, char, LexSubres<'a>, Simple<char>>,
    lp: char, rp: char
) -> impl Parser<char, LexSubres<'a>, Error=Simple<char>> + 'a {
    expr.padded().repeated()
    .map(|x| Box::new(x.into_iter().flatten()) as LexSubres)
    .delimited_by(just(lp), just(rp)).map_with_span(move |b, s| {
        Box::new(
            iter::once(Entry(Lexeme::LP(lp), s.start..s.start+1))
            .chain(b)
            .chain(iter::once(Entry(Lexeme::RP(lp), s.end-1..s.end)))
        ) as LexSubres
    })
}

pub fn lexer<'a, T: 'a>(ops: &[T]) -> impl Parser<char, LexedText, Error=Simple<char>> + 'a
where T: AsRef<str> + Clone {
    let all_ops = ops.iter().map(|o| o.as_ref().to_string())
        .chain(iter::once(".".to_string())).collect::<Vec<_>>();
    recursive(move |recurse: Recursive<char, LexSubres, Simple<char>>| {
        choice((
            paren_parser(recurse.clone(), '(', ')'),
            paren_parser(recurse.clone(), '[', ']'),
            paren_parser(recurse.clone(), '{', '}'),
            choice((
                just("==").padded().to(Lexeme::rule(0f64)),
                just("=").ignore_then(number::float_parser()).then_ignore(just("=>")).map(Lexeme::rule),
                comment::comment_parser().map(Lexeme::Comment),
                just("::").padded().to(Lexeme::NS),
                just('\\').padded().to(Lexeme::BS),
                just('@').padded().to(Lexeme::At),
                just(':').to(Lexeme::Type),
                number::int_parser().map(Lexeme::Int), // all ints are valid floats so it takes precedence
                number::float_parser().map(Lexeme::Num),
                string::char_parser().map(Lexeme::Char),
                string::str_parser().map(Lexeme::Str),
                name::name_parser(&all_ops).map(Lexeme::Name), // includes namespacing
            )).map_with_span(|lx, span| Box::new(iter::once(Entry(lx, span))) as LexSubres)
        ))
    }).separated_by(one_of("\t ").repeated())
    .flatten().collect()
    .separated_by(just('\n').then(text::whitespace()).ignored())
    .map(LexedText)
}
