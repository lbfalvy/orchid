use std::num::IntErrorKind;
use std::ops::Range;

use ordered_float::NotNan;
use rust_decimal::Decimal;

use crate::error::{mk_err, OrcErr};
use crate::intern;
use crate::location::Pos;

/// A number, either floating point or unsigned int, parsed by Orchid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
  /// A nonnegative integer
  Uint(u64),
  /// A binary float other than NaN
  Float(NotNan<f64>),
  /// A decimal number
  Decimal(Decimal),
}
impl Numeric {
  pub fn decimal(num: i64, scale: u32) -> Self { Self::Decimal(Decimal::new(num, scale)) }
  pub fn float(value: f64) -> Self { Self::Float(NotNan::new(value).unwrap()) }
}

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

pub fn num_to_err(NumError { kind, range }: NumError, offset: u32) -> OrcErr {
  mk_err(
    intern!(str: "Failed to parse number"),
    match kind {
      NumErrorKind::NaN => "NaN emerged during parsing",
      NumErrorKind::InvalidDigit => "non-digit character encountered",
      NumErrorKind::Overflow => "The number being described is too large or too accurate",
    },
    [Pos::Range(offset + range.start as u32..offset + range.end as u32).into()],
  )
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
        if let Some(radical) = u64::from(radix).checked_pow(pos_exp) {
          let number = base_usize.checked_mul(radical).ok_or(overflow_err)?;
          return Ok(Numeric::Uint(number));
        }
      }
      let f = (base_usize as f64) * (radix as f64).powi(exponent);
      let err = NumError { range: 0..string.len(), kind: NumErrorKind::NaN };
      Ok(Numeric::Float(NotNan::new(f).map_err(|_| err)?))
    },
    Some((whole, part)) => {
      let whole_n = int_parse(whole, radix, pos)?;
      let part_n = int_parse(part, radix, pos + whole.len() + 1)?;
      let scale = part.chars().filter(|c| *c != '_').count() as u32;
      if radix == 10 {
        let mut scaled_unit = Decimal::ONE;
        (scaled_unit.set_scale(scale))
          .map_err(|_| NumError { range: 0..string.len(), kind: NumErrorKind::Overflow })?;
        Ok(Numeric::Decimal(Decimal::from(whole_n) + scaled_unit * Decimal::from(part_n)))
      } else {
        let real_val = whole_n as f64 + (part_n as f64 / (radix as f64).powi(scale as i32));
        let f = real_val * (radix as f64).powi(exponent);
        Ok(Numeric::Float(NotNan::new(f).expect("None of the inputs are NaN")))
      }
    },
  }
}

fn int_parse(s: &str, radix: u8, start: usize) -> Result<u64, NumError> {
  let s = s.chars().filter(|c| *c != '_').collect::<String>();
  let range = start..(start + s.len());
  u64::from_str_radix(&s, radix as u32)
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

#[cfg(test)]
mod test {
  use super::{parse_num, Numeric};

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
    let test = |s, n| assert_eq!(parse_num(s), Ok(n));
    test("3.1417", Numeric::decimal(31417, 4));
    test("0xf.cafe", Numeric::float(0xf as f64 + 0xcafe as f64 / 0x10000 as f64));
    test("34p3", Numeric::Uint(34000));
    test("0x2p3", Numeric::Uint(0x2 * 0x1000));
    test("1.5p3", Numeric::decimal(1500, 0));
    test("0x2.5p3", Numeric::float((0x25 * 0x100) as f64));
  }
}
