use chumsky::{Parser, prelude::*};

use crate::ast::{Placeholder, PHClass};

use super::{number::int_parser, context::Context};

pub fn placeholder_parser<'a>(ctx: impl Context + 'a)
-> impl Parser<char, Placeholder, Error = Simple<char>> + 'a
{
  choice((
    just("...").to(Some(true)),
    just("..").to(Some(false)),
    empty().to(None)
  ))
  .then(just("$").ignore_then(text::ident()))
  .then(just(":").ignore_then(int_parser()).or_not())
  .try_map(move |((vec_nonzero, name), vec_prio), span| {
    let name = ctx.interner().i(&name);
    if let Some(nonzero) = vec_nonzero {
      let prio = vec_prio.unwrap_or_default();
      Ok(Placeholder { name, class: PHClass::Vec { nonzero, prio } })
    } else {
      if vec_prio.is_some() {
        Err(Simple::custom(span, "Scalar placeholders have no priority"))
      } else {
        Ok(Placeholder { name, class: PHClass::Scalar })
      }
    }
  })
}
