use chumsky::prelude::*;
use chumsky::{self, Parser};
use ordered_float::NotNan;

use super::decls::SimpleParser;

fn assert_not_digit(base: u32, c: char) {
  if base > (10 + (c as u32 - 'a' as u32)) {
    panic!("The character '{}' is a digit in base ({})", c, base)
  }
}

/// Parse an arbitrarily grouped sequence of digits starting with an underscore.
///
/// TODO: this should use separated_by and parse the leading group too
fn separated_digits_parser(base: u32) -> impl SimpleParser<char, String> {
  just('_')
    .ignore_then(text::digits(base))
    .repeated()
    .map(|sv| sv.iter().flat_map(|s| s.chars()).collect())
}

/// parse a grouped uint
///
/// Not to be confused with [int_parser] which does a lot more
fn uint_parser(base: u32) -> impl SimpleParser<char, u64> {
  text::int(base).then(separated_digits_parser(base)).map(
    move |(s1, s2): (String, String)| {
      u64::from_str_radix(&(s1 + &s2), base).unwrap()
    },
  )
}

/// parse exponent notation, or return 0 as the default exponent.
/// The exponent is always in decimal.
fn pow_parser() -> impl SimpleParser<char, i32> {
  choice((
    just('p').ignore_then(text::int(10)).map(|s: String| s.parse().unwrap()),
    just("p-")
      .ignore_then(text::int(10))
      .map(|s: String| -s.parse::<i32>().unwrap()),
  ))
  .or_else(|_| Ok(0))
}

/// returns a mapper that converts a mantissa and an exponent into an uint
///
/// TODO it panics if it finds a negative exponent
fn nat2u(base: u64) -> impl Fn((u64, i32)) -> u64 {
  move |(val, exp)| {
    if exp == 0 {
      val
    } else {
      val * base.checked_pow(exp.try_into().unwrap()).unwrap()
    }
  }
}

/// returns a mapper that converts a mantissa and an exponent into a float
fn nat2f(base: u64) -> impl Fn((NotNan<f64>, i32)) -> NotNan<f64> {
  move |(val, exp)| {
    if exp == 0 {
      val
    } else {
      val * (base as f64).powf(exp.try_into().unwrap())
    }
  }
}

/// parse an uint from exponential notation (panics if 'p' is a digit in base)
fn pow_uint_parser(base: u32) -> impl SimpleParser<char, u64> {
  assert_not_digit(base, 'p');
  uint_parser(base).then(pow_parser()).map(nat2u(base.into()))
}

/// parse an uint from a base determined by its prefix or lack thereof
///
/// Not to be confused with [uint_parser] which is a component of it.
pub fn int_parser() -> impl SimpleParser<char, u64> {
  choice((
    just("0b").ignore_then(pow_uint_parser(2)),
    just("0x").ignore_then(pow_uint_parser(16)),
    just('0').ignore_then(pow_uint_parser(8)),
    pow_uint_parser(10), // Dec has no prefix
  ))
}

/// parse a float from dot notation
fn dotted_parser(base: u32) -> impl SimpleParser<char, NotNan<f64>> {
  uint_parser(base)
    .then(
      just('.')
        .ignore_then(text::digits(base).then(separated_digits_parser(base)))
        .map(move |(frac1, frac2)| {
          let frac = frac1 + &frac2;
          let frac_num = u64::from_str_radix(&frac, base).unwrap() as f64;
          let dexp = base.pow(frac.len().try_into().unwrap());
          frac_num / dexp as f64
        })
        .or_not()
        .map(|o| o.unwrap_or_default()),
    )
    .try_map(|(wh, f), s| {
      NotNan::new(wh as f64 + f)
        .map_err(|_| Simple::custom(s, "Float literal evaluates to NaN"))
    })
}

/// parse a float from dotted and optionally also exponential notation
fn pow_float_parser(base: u32) -> impl SimpleParser<char, NotNan<f64>> {
  assert_not_digit(base, 'p');
  dotted_parser(base).then(pow_parser()).map(nat2f(base.into()))
}

/// parse a float with dotted and optionally exponential notation from a base
/// determined by its prefix
pub fn float_parser() -> impl SimpleParser<char, NotNan<f64>> {
  choice((
    just("0b").ignore_then(pow_float_parser(2)),
    just("0x").ignore_then(pow_float_parser(16)),
    just('0').ignore_then(pow_float_parser(8)),
    pow_float_parser(10),
  ))
  .labelled("float")
}

pub fn print_nat16(num: NotNan<f64>) -> String {
  let exp = num.log(16.0).floor();
  let man = num / 16_f64.powf(exp);
  format!("{man}p{exp:.0}")
}
