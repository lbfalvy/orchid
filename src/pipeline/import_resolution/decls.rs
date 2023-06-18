use trait_set::trait_set;

use crate::interner::Tok;

trait_set! {
  pub trait InjectedAsFn = Fn(&[Tok<String>]) -> Option<Vec<Tok<String>>>;
  pub trait UpdatedFn = Fn(&[Tok<String>]) -> bool;
}
