use chumsky::{Parser, prelude::*};
use itertools::Itertools;
use crate::representations::sourcefile::Import;
use crate::utils::iter::{box_once, box_flatten, into_boxed_iter, BoxedIterIter};
use crate::interner::Token;
use crate::{box_chain, enum_filter};

use super::Entry;
use super::context::Context;
use super::lexer::{Lexeme, filter_map_lex};

/// initialize a BoxedIter<BoxedIter<String>> with a single element.
fn init_table(name: Token<String>) -> BoxedIterIter<'static, Token<String>> {
  // I'm not at all confident that this is a good approach.
  box_once(box_once(name))
}

/// Parse an import command
/// Syntax is same as Rust's `use` except the verb is import, no trailing
/// semi and the delimiters are plain parentheses. Namespaces should
/// preferably contain crossplatform filename-legal characters but the
/// symbols are explicitly allowed to go wild.
/// There's a blacklist in [name]
pub fn import_parser<'a>(ctx: impl Context + 'a)
-> impl Parser<Entry, Vec<Import>, Error = Simple<Entry>> + 'a
{
  // TODO: this algorithm isn't cache friendly and copies a lot
  recursive({
    let ctx = ctx.clone();
    move |expr:Recursive<Entry, BoxedIterIter<Token<String>>, Simple<Entry>>| {
      filter_map_lex(enum_filter!(Lexeme::Name)).map(|(t, _)| t)
      .separated_by(Lexeme::NS.parser())
      .then(
        Lexeme::NS.parser()
        .ignore_then(
          choice((
            expr.clone()
              .separated_by(Lexeme::Name(ctx.interner().i(",")).parser())
              .delimited_by(Lexeme::LP('(').parser(), Lexeme::RP('(').parser())
              .map(|v| box_flatten(v.into_iter()))
              .labelled("import group"),
            // Each expr returns a list of imports, flatten into common list
            Lexeme::Name(ctx.interner().i("*")).parser()
              .map(move |_| init_table(ctx.interner().i("*")))
              .labelled("wildcard import"), // Just a *, wrapped
            filter_map_lex(enum_filter!(Lexeme::Name))
              .map(|(t, _)| init_table(t))
              .labelled("import terminal") // Just a name, wrapped
          ))
        ).or_not()
      )
      .map(|(name, opt_post): (Vec<Token<String>>, Option<BoxedIterIter<Token<String>>>)|
      -> BoxedIterIter<Token<String>> {
        if let Some(post) = opt_post {
          Box::new(post.map(move |el| {
            box_chain!(name.clone().into_iter(), el)
          }))
        } else {
          box_once(into_boxed_iter(name))
        }
      })
    }
  }).map(move |paths| {
    paths.filter_map(|namespaces| {
      let mut path = namespaces.collect_vec();
      let name = path.pop()?;
      Some(Import {
        path: ctx.interner().i(&path),
        name: {
          if name == ctx.interner().i("*") { None }
          else { Some(name) }
        }
      })
    }).collect()
  }).labelled("import")
}
