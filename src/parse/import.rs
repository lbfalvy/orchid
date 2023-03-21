use std::rc::Rc;

use chumsky::{Parser, prelude::*};
use itertools::Itertools;
use lasso::Spur;
use crate::representations::sourcefile::Import;
use crate::utils::iter::{box_once, box_flatten, into_boxed_iter, BoxedIterIter};
use crate::{enum_parser, box_chain};

use super::lexer::Lexeme;

/// initialize a BoxedIter<BoxedIter<String>> with a single element.
fn init_table(name: Spur) -> BoxedIterIter<'static, Spur> {
  // I'm not at all confident that this is a good approach.
  box_once(box_once(name))
}

/// Parse an import command
/// Syntax is same as Rust's `use` except the verb is import, no trailing
/// semi and the delimiters are plain parentheses. Namespaces should
/// preferably contain crossplatform filename-legal characters but the
/// symbols are explicitly allowed to go wild.
/// There's a blacklist in [name]
pub fn import_parser<'a, F>(intern: &'a F)
-> impl Parser<Lexeme, Vec<Import>, Error = Simple<Lexeme>> + 'a
where F: Fn(&str) -> Spur + 'a {
  let globstar = intern("*");
  // TODO: this algorithm isn't cache friendly and copies a lot
  recursive(move |expr:Recursive<Lexeme, BoxedIterIter<Spur>, Simple<Lexeme>>| {
    enum_parser!(Lexeme::Name).map(|s| intern(s.as_str()))
    .separated_by(just(Lexeme::NS))
    .then(
      just(Lexeme::NS)
      .ignore_then(
        choice((
          expr.clone()
            .separated_by(just(Lexeme::name(",")))
            .delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
            .map(|v| box_flatten(v.into_iter()))
            .labelled("import group"),
          // Each expr returns a list of imports, flatten into common list
          just(Lexeme::name("*")).map(move |_| init_table(globstar))
            .labelled("wildcard import"), // Just a *, wrapped
          enum_parser!(Lexeme::Name)
            .map(|s| init_table(intern(s.as_str())))
            .labelled("import terminal") // Just a name, wrapped
        ))
      ).or_not()
    )
    .map(|(name, opt_post): (Vec<Spur>, Option<BoxedIterIter<Spur>>)|
    -> BoxedIterIter<Spur> {
      if let Some(post) = opt_post {
        Box::new(post.map(move |el| {
          box_chain!(name.clone().into_iter(), el)
        }))
      } else {
        box_once(into_boxed_iter(name))
      }
    })
  }).map(move |paths| {
    paths.filter_map(|namespaces| {
      let mut path = namespaces.collect_vec();
      let name = path.pop()?;
      Some(Import {
        path: Rc::new(path),
        name: {
          if name == globstar { None }
          else { Some(name.to_owned()) }
        }
      })
    }).collect()
  }).labelled("import")
}
