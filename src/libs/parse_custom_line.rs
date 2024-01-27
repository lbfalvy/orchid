//! A helper for defining custom lines. See [custom_line]
use intern_all::Tok;

use crate::error::ProjectResult;
use crate::location::SourceRange;
use crate::parse::errors::ParseErrorKind;
use crate::parse::frag::Frag;
use crate::parse::lexer::Lexeme;
use crate::parse::parse_plugin::ParsePluginReq;

/// An exported line with a name for which the line parser denies exports
pub struct Unexportable(Lexeme);
impl ParseErrorKind for Unexportable {
  const DESCRIPTION: &'static str = "this line type cannot be exported";
  fn message(&self) -> String { format!("{} cannot be exported", &self.0) }
}

/// Parse a line identified by the specified leading keyword. Although not
/// required, plugins are encouraged to prefix their lines with a globally
/// unique keyword which makes or breaks their parsing, to avoid accidental
/// failure to recognize
pub fn custom_line<'a>(
  tail: Frag<'a>,
  keyword: Tok<String>,
  exportable: bool,
  req: &dyn ParsePluginReq,
) -> Option<ProjectResult<(bool, Frag<'a>, SourceRange)>> {
  let line_loc = req.frag_loc(tail);
  let (fst, tail) = req.pop(tail).ok()?;
  let fst_name = req.expect_name(fst).ok()?;
  let (exported, n_ent, tail) = if fst_name == keyword {
    (false, fst, tail.trim())
  } else if fst_name.as_str() == "export" {
    let (snd, tail) = req.pop(tail).ok()?;
    req.expect(Lexeme::Name(keyword), snd).ok()?;
    (true, snd, tail.trim())
  } else {
    return None;
  };
  Some(match exported && !exportable {
    true => {
      let err = Unexportable(n_ent.lexeme.clone());
      Err(err.pack(req.range_loc(n_ent.range.clone())))
    },
    false => Ok((exported, tail, line_loc)),
  })
}
