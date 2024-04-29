//! Parse a float or integer. These functions are also used for the macro
//! priority numbers

use std::num::IntErrorKind;
use std::ops::Range;

use ordered_float::NotNan;

use super::context::ParseCtx;
use super::errors::{ExpectedDigit, LiteralOverflow, NaNLiteral, ParseErrorKind};
use super::lex_plugin::LexPluginReq;
#[allow(unused)] // for doc
use super::lex_plugin::LexerPlugin;
use super::lexer::{split_filter, Entry, LexRes, Lexeme};
use crate::error::{ProjectErrorObj, ProjectResult};
use crate::foreign::atom::AtomGenerator;
use crate::foreign::inert::Inert;
use crate::libs::std::number::Numeric;

/// Rasons why [parse_num] might fail. See [NumError].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumErrorKind {
  /// The literal describes [f64::NAN]
  NaN,
  /// Some integer appearing in the literal overflows [usize]
  Overflow,
  /// A character that isn't a digit in the given base was found
  InvalidDigit,
}
impl NumErrorKind {
  fn from_int(kind: &IntErrorKind) -> Self {
    match kind {
      IntErrorKind::InvalidDigit => Self::InvalidDigit,
      IntErrorKind::NegOverflow | IntErrorKind::PosOverflow => Self::Overflow,
      _ => panic!("Impossible error condition"),
    }
  }
}

/// Error produced by [parse_num]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumError {
  /// Location
  pub range: Range<usize>,
  /// Reason
  pub kind: NumErrorKind,
}

impl NumError {
  /// Convert into [ProjectErrorObj]
  pub fn into_proj(
    self,
    len: usize,
    tail: &str,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectErrorObj {
    let start = ctx.source().len() - tail.len() - len + self.range.start;
    let location = ctx.range_loc(&(start..start + self.range.len()));
    match self.kind {
      NumErrorKind::NaN => NaNLiteral.pack(location),
      NumErrorKind::InvalidDigit => ExpectedDigit.pack(location),
      NumErrorKind::Overflow => LiteralOverflow.pack(location),
    }
  }
}

/// Parse a numbre literal out of text
pub fn parse_num(string: &str) -> Result<Numeric, NumError> {
  let overflow_err = NumError { range: 0..string.len(), kind: NumErrorKind::Overflow };
  let (radix, noprefix, pos) = (string.strip_prefix("0x").map(|s| (16u8, s, 2)))
    .or_else(|| string.strip_prefix("0b").map(|s| (2u8, s, 2)))
    .or_else(|| string.strip_prefix("0o").map(|s| (8u8, s, 2)))
    .unwrap_or((10u8, string, 0));
  // identity
  let (base, exponent) = match noprefix.split_once('p') {
    Some((b, e)) => {
      let (s, d, len) = e.strip_prefix('-').map_or((1, e, 0), |ue| (-1, ue, 1));
      (b, s * int_parse(d, 10, pos + b.len() + 1 + len)? as i32)
    },
    None => (noprefix, 0),
  };
  match base.split_once('.') {
    None => {
      let base_usize = int_parse(base, radix, pos)?;
      if let Ok(pos_exp) = u32::try_from(exponent) {
        if let Some(radical) = usize::from(radix).checked_pow(pos_exp) {
          let number = base_usize.checked_mul(radical).ok_or(overflow_err)?;
          return Ok(Numeric::Uint(number));
        }
      }
      let f = (base_usize as f64) * (radix as f64).powi(exponent);
      let err = NumError { range: 0..string.len(), kind: NumErrorKind::NaN };
      Ok(Numeric::Float(NotNan::new(f).map_err(|_| err)?))
    },
    Some((whole, part)) => {
      let whole_n = int_parse(whole, radix, pos)? as f64;
      let part_n = int_parse(part, radix, pos + whole.len() + 1)? as f64;
      let real_val = whole_n + (part_n / (radix as f64).powi(part.len() as i32));
      let f = real_val * (radix as f64).powi(exponent);
      Ok(Numeric::Float(NotNan::new(f).expect("None of the inputs are NaN")))
    },
  }
}

fn int_parse(s: &str, radix: u8, start: usize) -> Result<usize, NumError> {
  let s = s.chars().filter(|c| *c != '_').collect::<String>();
  let range = start..(start + s.len());
  usize::from_str_radix(&s, radix as u32)
    .map_err(|e| NumError { range, kind: NumErrorKind::from_int(e.kind()) })
}

/// Filter for characters that can appear in numbers
pub fn numchar(c: char) -> bool { c.is_alphanumeric() | "._-".contains(c) }
/// Filter for characters that can start numbers
pub fn numstart(c: char) -> bool { c.is_ascii_digit() }

/// Print a number as a base-16 floating point literal
#[must_use]
pub fn print_nat16(num: NotNan<f64>) -> String {
  if *num == 0.0 {
    return "0x0".to_string();
  } else if num.is_infinite() {
    return match num.is_sign_positive() {
      true => "Infinity".to_string(),
      false => "-Infinity".to_string(),
    };
  } else if num.is_nan() {
    return "NaN".to_string();
  }
  let exp = num.log(16.0).floor();
  let man = *num / 16_f64.powf(exp);
  format!("0x{man}p{exp:.0}")
}

/// [LexerPlugin] for a number literal
#[derive(Clone)]
pub struct NumericLexer;
impl LexerPlugin for NumericLexer {
  fn lex<'b>(&self, req: &'_ dyn LexPluginReq<'b>) -> Option<ProjectResult<LexRes<'b>>> {
    req.tail().chars().next().filter(|c| numstart(*c)).map(|_| {
      let (num_str, tail) = split_filter(req.tail(), numchar);
      let ag = match parse_num(num_str) {
        Ok(Numeric::Float(f)) => AtomGenerator::cloner(Inert(f)),
        Ok(Numeric::Uint(i)) => AtomGenerator::cloner(Inert(i)),
        Err(e) => return Err(e.into_proj(num_str.len(), tail, req.ctx())),
      };
      let range = req.ctx().range(num_str.len(), tail);
      let entry = Entry { lexeme: Lexeme::Atom(ag), range };
      Ok(LexRes { tail, tokens: vec![entry] })
    })
  }
}

#[cfg(test)]
mod test {
  use crate::libs::std::number::Numeric;
  use crate::parse::numeric::parse_num;

  #[test]
  fn just_ints() {
    let test = |s, n| assert_eq!(parse_num(s), Ok(Numeric::Uint(n)));
    test("12345", 12345);
    test("0xcafebabe", 0xcafebabe);
    test("0o751", 0o751);
    test("0b111000111", 0b111000111);
  }

  #[test]
  fn decimals() {
    let test = |s, n| assert_eq!(parse_num(s).map(|n| n.as_f64()), Ok(n));
    test("3.1417", 3.1417);
    test("3.1417", 3_f64 + 1417_f64 / 10000_f64);
    test("0xf.cafe", 0xf as f64 + 0xcafe as f64 / 0x10000 as f64);
    test("34p3", 34000f64);
    test("0x2p3", (0x2 * 0x1000) as f64);
    test("1.5p3", 1500f64);
    test("0x2.5p3", (0x25 * 0x100) as f64);
  }
}
