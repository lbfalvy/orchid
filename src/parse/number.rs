use chumsky::{self, prelude::*, Parser};

fn assert_not_digit(base: u32, c: char) {
    if base > (10 + (c as u32 - 'a' as u32)) {
        panic!("The character '{}' is a digit in base ({})", c, base)
    }
}

fn separated_digits_parser(base: u32) -> impl Parser<char, String, Error = Simple<char>> {
    just('_')
        .ignore_then(text::digits(base))
        .repeated()
        .map(|sv| sv.iter().map(|s| s.chars()).flatten().collect())
}

fn uint_parser(base: u32) -> impl Parser<char, u64, Error = Simple<char>> {
    text::int(base)
        .then(separated_digits_parser(base))
        .map(move |(s1, s2): (String, String)| {
            u64::from_str_radix(&(s1 + &s2), base).unwrap()
        })
}

fn pow_parser() -> impl Parser<char, i32, Error = Simple<char>> {
    return choice((
        just('p')
            .ignore_then(text::int(10))
            .map(|s: String| s.parse().unwrap()),
        just("p-")
            .ignore_then(text::int(10))
            .map(|s: String| -s.parse::<i32>().unwrap()),
    )).or_else(|_| Ok(0))
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

fn pow_uint_parser(base: u32) -> impl Parser<char, u64, Error = Simple<char>> {
    assert_not_digit(base, 'p');
    uint_parser(base).then(pow_parser()).map(nat2u(base.into()))
}

pub fn int_parser() -> impl Parser<char, u64, Error = Simple<char>> {
    choice((
        just("0b").ignore_then(pow_uint_parser(2)),
        just("0x").ignore_then(pow_uint_parser(16)),
        just('0').ignore_then(pow_uint_parser(8)),
        pow_uint_parser(10), // Dec has no prefix
    ))
}

fn dotted_parser(base: u32) -> impl Parser<char, f64, Error = Simple<char>> {
    uint_parser(base)
    .then_ignore(just('.'))
    .then(
        text::digits(base).then(separated_digits_parser(base))
    ).map(move |(wh, (frac1, frac2))| {
        let frac = frac1 + &frac2;
        let frac_num = u64::from_str_radix(&frac, base).unwrap() as f64;
        let dexp = base.pow(frac.len().try_into().unwrap());
        wh as f64 + (frac_num / dexp as f64)
    })
}

fn pow_float_parser(base: u32) -> impl Parser<char, f64, Error = Simple<char>> {
    assert_not_digit(base, 'p');
    dotted_parser(base).then(pow_parser()).map(nat2f(base.into()))
}

pub fn float_parser() -> impl Parser<char, f64, Error = Simple<char>> {
    choice((
        just("0b").ignore_then(pow_float_parser(2)),
        just("0x").ignore_then(pow_float_parser(16)),
        just('0').ignore_then(pow_float_parser(8)),
        pow_float_parser(10),
    ))
}
