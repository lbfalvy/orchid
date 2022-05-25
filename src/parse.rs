use std::fmt::Debug;
use chumsky::{self, prelude::*, Parser};

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Int(u64),
    Char(char),
    Str(String),
    Name(String),
    S(Vec<Expr>),
    Lambda(String, Vec<Expr>)
}

fn uint_parser(base: u32) -> impl Parser<char, u64, Error = Simple<char>> {
    text::int(base).map(move |s: String| u64::from_str_radix(&s, base).unwrap())
}

fn e_parser() -> impl Parser<char, i32, Error = Simple<char>> {
    return choice((
        just('e')
            .ignore_then(text::int(10))
            .map(|s: String| s.parse().unwrap()),
        just("e-")
            .ignore_then(text::int(10))
            .map(|s: String| -s.parse::<i32>().unwrap()),
        empty().map(|()| 0)
    ))
}

fn nat2u(base: u64) -> impl Fn((u64, i32),) -> u64 {
    return move |(val, exp)| {
        if exp == 0 {val}
        else {val * base.checked_pow(exp.try_into().unwrap()).unwrap()}
    };
}

fn nat2f(base: u64) -> impl Fn((f64, i32),) -> f64 {
    return move |(val, exp)| {
        if exp == 0 {val}
        else {val * (base as f64).powf(exp.try_into().unwrap())}
    }
}

fn e_uint_parser(base: u32) -> impl Parser<char, u64, Error = Simple<char>> {
    if base > 14 {panic!("exponential in base that uses the digit 'e' is ambiguous")}
    uint_parser(base).then(e_parser()).map(nat2u(base.into()))
}

fn int_parser() -> impl Parser<char, u64, Error = Simple<char>> {
    choice((
        just("0b").ignore_then(e_uint_parser(2)),
        just("0x").ignore_then(uint_parser(16)),
        just('0').ignore_then(e_uint_parser(8)),
        e_uint_parser(10), // Dec has no prefix
    ))
}

fn dotted_parser(base: u32) -> impl Parser<char, f64, Error = Simple<char>> {
    uint_parser(base)
    .then_ignore(just('.'))
    .then(text::digits(base))
    .map(move |(wh, frac)| {
        let frac_num = u64::from_str_radix(&frac, base).unwrap() as f64;
        let dexp = base.pow(frac.len().try_into().unwrap());
        wh as f64 + (frac_num / dexp as f64)
    })
}

fn e_float_parser(base: u32) -> impl Parser<char, f64, Error = Simple<char>> {
    if base > 14 {panic!("exponential in base that uses the digit 'e' is ambiguous")}
    dotted_parser(base).then(e_parser()).map(nat2f(base.into()))
}

fn float_parser() -> impl Parser<char, f64, Error = Simple<char>> {
    choice((
        just("0b").ignore_then(e_float_parser(2)),
        just("0x").ignore_then(dotted_parser(16)),
        just('0').ignore_then(e_float_parser(8)),
        e_float_parser(10),
    ))
}

fn text_parser(delim: char) -> impl Parser<char, char, Error = Simple<char>> {
    let escape = just('\\').ignore_then(
        just('\\')
            .or(just('/'))
            .or(just('"'))
            .or(just('b').to('\x08'))
            .or(just('f').to('\x0C'))
            .or(just('n').to('\n'))
            .or(just('r').to('\r'))
            .or(just('t').to('\t'))
            .or(just('u').ignore_then(
                filter(|c: &char| c.is_digit(16))
                    .repeated()
                    .exactly(4)
                    .collect::<String>()
                    .validate(|digits, span, emit| {
                        char::from_u32(u32::from_str_radix(&digits, 16).unwrap())
                            .unwrap_or_else(|| {
                                emit(Simple::custom(span, "invalid unicode character"));
                                '\u{FFFD}' // unicode replacement character
                            })
                    }),
            )),
    );
    filter(move |&c| c != '\\' && c != delim).or(escape)
}

fn char_parser() -> impl Parser<char, char, Error = Simple<char>> {
    just('\'').ignore_then(text_parser('\'')).then_ignore(just('\''))
}

fn str_parser() -> impl Parser<char, String, Error = Simple<char>> {
    just('"')
    .ignore_then(text_parser('"').repeated())
    .then_ignore(just('"'))
    .collect()
}

pub fn parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    return recursive(|expr| {
        let lambda = just('\\')
            .ignore_then(text::ident())
            .then_ignore(just('.'))
            .then(expr.clone().repeated().at_least(1))
            .map(|(name, body)| Expr::Lambda(name, body));
        let sexpr = expr.clone()
            .repeated()
            .delimited_by(just('('), just(')'))
            .map(Expr::S);
        choice((
            float_parser().map(Expr::Num),
            int_parser().map(Expr::Int),
            char_parser().map(Expr::Char),
            str_parser().map(Expr::Str),
            text::ident().map(Expr::Name),
            sexpr,
            lambda
        )).padded()
    }).then_ignore(end())
}