use std::ops::RangeInclusive;

use orchid_base::location::Pos;
use orchid_base::number::{parse_num, NumError, NumErrorKind, Numeric};
use orchid_extension::atom::AtomicFeatures;
use orchid_extension::error::{ProjectError, ProjectResult};
use orchid_extension::lexer::{LexContext, Lexer};
use orchid_extension::tree::{GenTok, GenTokTree};
use ordered_float::NotNan;

use super::num_atom::{Float, Int};

struct NumProjError(u32, NumError);
impl ProjectError for NumProjError {
  const DESCRIPTION: &'static str = "Failed to parse number";
  fn message(&self) -> String {
    match self.1.kind {
      NumErrorKind::InvalidDigit => "This character is not meaningful in this base",
      NumErrorKind::NaN => "Number somehow evaluated to NaN",
      NumErrorKind::Overflow => "Number literal overflowed its enclosing type",
    }
    .to_string()
  }
  fn one_position(&self) -> Pos {
    Pos::Range(self.0 + self.1.range.start as u32..self.0 + self.1.range.end as u32)
  }
}

#[derive(Default)]
pub struct NumLexer;
impl Lexer for NumLexer {
  const CHAR_FILTER: &'static [RangeInclusive<char>] = &['0'..='9'];
  fn lex<'a>(all: &'a str, ctx: &'a LexContext<'a>) -> ProjectResult<(&'a str, GenTokTree)> {
    let ends_at = all.find(|c: char| !c.is_ascii_hexdigit() && !"xX._pP".contains(c));
    let (chars, tail) = all.split_at(ends_at.unwrap_or(all.len()));
    let fac = match parse_num(chars) {
      Ok(Numeric::Float(f)) => Float(f).factory(),
      Ok(Numeric::Uint(uint)) => Int(uint.try_into().unwrap()).factory(),
      Ok(Numeric::Decimal(dec)) => Float(NotNan::new(dec.try_into().unwrap()).unwrap()).factory(),
      Err(e) => return Err(NumProjError(ctx.pos(all), e).pack()),
    };
    Ok((tail, GenTok::Atom(fac).at(ctx.pos(all)..ctx.pos(tail))))
  }
}
