use std::fmt::Write;

use intern_all::i;

use crate::error::ProjectResult;
use crate::foreign::atom::AtomGenerator;
use crate::foreign::inert::Inert;
use crate::libs::std::string::OrcString;
use crate::parse::errors::ParseErrorKind;
use crate::parse::lex_plugin::{LexPluginRecur, LexPluginReq, LexerPlugin};
use crate::parse::lexer::{Entry, LexRes, Lexeme};
use crate::parse::parsed::PType;
use crate::parse::string::parse_string;

pub struct TStringLexer;
impl LexerPlugin for TStringLexer {
  fn lex<'a>(&self, req: &'_ dyn LexPluginReq<'a>) -> Option<ProjectResult<LexRes<'a>>> {
    req.tail().strip_prefix("$\"").map(|mut txt| {
      let ctx = req.ctx();
      let mut parts = vec![Entry::new(ctx.range(0, txt), Lexeme::LP(PType::Par))];
      let mut str = String::new();
      let commit_str = |str: &mut String, tail: &str, parts: &mut Vec<Entry>| -> ProjectResult<_> {
        let str_val = parse_string(str).map_err(|e| e.to_proj(ctx, ctx.pos(txt)))?;
        let ag = AtomGenerator::cloner(Inert(OrcString::from(i(&str_val))));
        parts.push(Entry::new(ctx.range(str.len(), tail), Lexeme::Atom(ag)));
        *str = String::new();
        Ok(())
      };
      loop {
        if let Some(rest) = txt.strip_prefix('"') {
          commit_str(&mut str, txt, &mut parts)?;
          parts.push(Entry::new(ctx.range(0, rest), Lexeme::RP(PType::Par)));
          return Ok(LexRes { tail: rest, tokens: parts });
        }
        if let Some(rest) = txt.strip_prefix("${") {
          let mut depth = 0;
          commit_str(&mut str, rest, &mut parts)?;
          parts.extend(req.insert("++ std::conv::to_string (", ctx.source_range(0, rest)));
          let res = req.recurse(LexPluginRecur {
            tail: rest,
            exit: &mut |c| {
              match c.chars().next() {
                None => return Err(UnclosedInterpolation.pack(ctx.source_range(2, rest))),
                Some('{') => depth += 1,
                Some('}') if depth == 0 => return Ok(true),
                Some('}') => depth -= 1,
                _ => (),
              }
              Ok(false)
            },
          })?;
          txt = &res.tail[1..]; // account for final }
          parts.extend(res.tokens);
          parts.extend(req.insert(") ++", ctx.source_range(0, txt)));
        } else {
          let mut chars = txt.chars();
          match chars.next() {
            None => return Err(NoTStringEnd.pack(ctx.source_range(req.tail().len(), ""))),
            Some('\\') => match chars.next() {
              None => write!(str, "\\").expect("writing \\ into string"),
              Some(next) => write!(str, "\\{next}").expect("writing \\ and char into string"),
            },
            Some(c) => write!(str, "{c}").expect("writing char into string"),
          }
          txt = chars.as_str();
        }
      }
    })
  }
}

pub struct UnclosedInterpolation;
impl ParseErrorKind for UnclosedInterpolation {
  const DESCRIPTION: &'static str = "A ${ block within a $-string wasn't closed";
}

/// String literal never ends
pub(super) struct NoTStringEnd;
impl ParseErrorKind for NoTStringEnd {
  const DESCRIPTION: &'static str = "A $-string literal was not closed with `\"`";
}
