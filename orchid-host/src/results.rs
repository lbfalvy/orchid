use std::sync::Arc;

use orchid_base::error::{ErrorPosition, OwnedError};
use orchid_base::intern;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::number::{NumError, NumErrorKind};

pub type OwnedResult<T> = Result<T, Vec<OwnedError>>;

pub fn mk_err(
  description: Tok<String>,
  message: impl AsRef<str>,
  posv: impl IntoIterator<Item = ErrorPosition>,
) -> OwnedError {
  OwnedError {
    description,
    message: Arc::new(message.as_ref().to_string()),
    positions: posv.into_iter().collect(),
  }
}

pub fn num_to_err(NumError { kind, range }: NumError, offset: u32) -> OwnedError {
  OwnedError {
    description: intern!(str: "Failed to parse number"),
    message: Arc::new(
      match kind {
        NumErrorKind::NaN => "NaN emerged during parsing",
        NumErrorKind::InvalidDigit => "non-digit character encountered",
        NumErrorKind::Overflow => "The number being described is too large or too accurate",
      }
      .to_string(),
    ),
    positions: vec![Pos::Range(offset + range.start as u32..offset + range.end as u32).into()],
  }
}
