//! A helper for defining custom lines. See [custom_line]
use crate::error::{ProjectError, ProjectResult};
use crate::parse::errors::{Expected, ExpectedName};
use crate::parse::{Entry, Lexeme, Stream};
use crate::{Location, Tok};

/// An exported line with a name for which the line parser denies exports
pub struct Unexportable(Entry);
impl ProjectError for Unexportable {
  fn description(&self) -> &str { "this line type cannot be exported" }
  fn message(&self) -> String { format!("{} cannot be exported", &self.0) }
  fn one_position(&self) -> Location { self.0.location() }
}

/// Parse a line identified by the specified leading keyword. Although not
/// required, plugins are encouraged to prefix their lines with a globally
/// unique keyword which makes or breaks their parsing, to avoid accidental
/// failure to recognize 
pub fn custom_line(
  tail: Stream<'_>,
  keyword: Tok<String>,
  exportable: bool,
) -> Option<ProjectResult<(bool, Stream<'_>, Location)>> {
  let line_loc = tail.location();
  let (fst, tail) = tail.pop().ok()?;
  let fst_name = ExpectedName::expect(fst).ok()?;
  let (exported, n_ent, tail) = if fst_name == keyword {
    (false, fst, tail.trim())
  } else if fst_name.as_str() == "export" {
    let (snd, tail) = tail.pop().ok()?;
    Expected::expect(Lexeme::Name(keyword), snd).ok()?;
    (true, snd, tail.trim())
  } else {
    return None;
  };
  Some(match exported && !exportable {
    true => Err(Unexportable(n_ent.clone()).rc()),
    false => Ok((exported, tail, line_loc)),
  })
}
