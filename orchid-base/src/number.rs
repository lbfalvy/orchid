use std::num::IntErrorKind;
use std::ops::Range;

use ordered_float::NotNan;

use crate::error::{OrcErr, mk_err};
use crate::interner::Interner;
use crate::location::SrcRange;
use crate::name::Sym;

/// A number, either floating point or unsigned int, parsed by Orchid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
	/// An integer
	Int(i64),
	/// A binary float other than NaN
	Float(NotNan<f64>),
}
impl Numeric {
	pub fn float(value: f64) -> Self { Self::Float(NotNan::new(value).unwrap()) }
	pub fn to_f64(self) -> NotNan<f64> {
		match self {
			Self::Float(f) => f,
			Self::Int(i) => NotNan::new(i as f64).expect("int cannot be NaN"),
		}
	}
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

pub async fn num_to_err(
	NumError { kind, range }: NumError,
	offset: u32,
	source: &Sym,
	i: &Interner,
) -> OrcErr {
	mk_err(
		i.i("Failed to parse number").await,
		match kind {
			NumErrorKind::NaN => "NaN emerged during parsing",
			NumErrorKind::InvalidDigit => "non-digit character encountered",
			NumErrorKind::Overflow => "The number being described is too large or too accurate",
		},
		[SrcRange::new(offset + range.start as u32..offset + range.end as u32, source).pos().into()],
	)
}

/// Parse a numbre literal out of text
pub fn parse_num(string: &str) -> Result<Numeric, NumError> {
	let overflow_e = NumError { range: 0..string.len(), kind: NumErrorKind::Overflow };
	let (radix, noprefix, pos) = (string.strip_prefix("0x").map(|s| (16u8, s, 2)))
		.or_else(|| string.strip_prefix("0b").map(|s| (2u8, s, 2)))
		.or_else(|| string.strip_prefix("0o").map(|s| (8u8, s, 2)))
		.unwrap_or((10u8, string, 0));
	eprintln!("({radix}, {noprefix}, {pos})");
	// identity
	let (base_s, exponent) = match noprefix.split_once('p') {
		Some((b, e)) => {
			let (s, d, len) = e.strip_prefix('-').map_or((1, e, 0), |ue| (-1, ue, 1));
			(b, s * int_parse(d, 10, pos + b.len() + 1 + len)? as i32)
		},
		None => (noprefix, 0),
	};
	eprintln!("({base_s},{exponent})");
	match base_s.split_once('.') {
		None => {
			let base = int_parse(base_s, radix, pos)?;
			if let Ok(pos_exp) = u32::try_from(exponent) {
				if let Some(radical) = u64::from(radix).checked_pow(pos_exp) {
					let num = base.checked_mul(radical).and_then(|m| m.try_into().ok()).ok_or(overflow_e)?;
					return Ok(Numeric::Int(num));
				}
			}
			let f = (base as f64) * (radix as f64).powi(exponent);
			let err = NumError { range: 0..string.len(), kind: NumErrorKind::NaN };
			Ok(Numeric::Float(NotNan::new(f).map_err(|_| err)?))
		},
		Some((whole, part)) => {
			let whole_n = int_parse(whole, radix, pos)?;
			let part_n = int_parse(part, radix, pos + whole.len() + 1)?;
			let scale = part.chars().filter(|c| *c != '_').count() as u32;
			let real_val = whole_n as f64 + (part_n as f64 / (radix as f64).powi(scale as i32));
			let f = real_val * (radix as f64).powi(exponent);
			Ok(Numeric::Float(NotNan::new(f).expect("None of the inputs are NaN")))
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
	use super::{Numeric, parse_num};

	#[test]
	fn just_ints() {
		let test = |s, n| assert_eq!(parse_num(s), Ok(Numeric::Int(n)));
		test("12345", 12345);
		test("0xcafebabe", 0xcafebabe);
		test("0o751", 0o751);
		test("0b111000111", 0b111000111);
	}

	#[test]
	fn decimals() {
		let test = |s, n| assert_eq!(parse_num(s), Ok(n));
		test("3.1417", Numeric::float(3.1417));
		test("0xf.cafe", Numeric::float(0xf as f64 + 0xcafe as f64 / 0x10000 as f64));
		test("34p3", Numeric::Int(34000));
		test("0x2p3", Numeric::Int(0x2 * 0x1000));
		test("1.5p3", Numeric::float(1500.0));
		test("0x2.5p3", Numeric::float((0x25 * 0x100) as f64));
	}
}
