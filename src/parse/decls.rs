use std::hash::Hash;

use chumsky::prelude::Simple;
use chumsky::recursive::Recursive;
use chumsky::{BoxedParser, Parser};
use trait_set::trait_set;

trait_set! {
  /// Wrapper around [Parser] with [Simple] error to avoid repeating the input
  pub trait SimpleParser<I: Eq + Hash + Clone, O> =
    Parser<I, O, Error = Simple<I>>;
}
/// Boxed version of [SimpleParser]
pub type BoxedSimpleParser<'a, I, O> = BoxedParser<'a, I, O, Simple<I>>;
/// [Recursive] specialization of [SimpleParser] to parameterize calls to
/// [chumsky::recursive::recursive]
pub type SimpleRecursive<'a, I, O> = Recursive<'a, I, O, Simple<I>>;
