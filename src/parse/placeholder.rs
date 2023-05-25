use chumsky::prelude::*;
use chumsky::Parser;

use super::context::Context;
use super::decls::SimpleParser;
use super::number::int_parser;
use crate::ast::{PHClass, Placeholder};

pub fn placeholder_parser(
  ctx: impl Context,
) -> impl SimpleParser<char, Placeholder> {
  choice((
    just("...").to(Some(true)),
    just("..").to(Some(false)),
    empty().to(None),
  ))
  .then(just("$").ignore_then(text::ident()))
  .then(just(":").ignore_then(int_parser()).or_not())
  .try_map(move |((vec_nonzero, name), vec_prio), span| {
    let name = ctx.interner().i(&name);
    if let Some(nonzero) = vec_nonzero {
      let prio = vec_prio.unwrap_or_default();
      Ok(Placeholder { name, class: PHClass::Vec { nonzero, prio } })
    } else if vec_prio.is_some() {
      Err(Simple::custom(span, "Scalar placeholders have no priority"))
    } else {
      Ok(Placeholder { name, class: PHClass::Scalar })
    }
  })
}
