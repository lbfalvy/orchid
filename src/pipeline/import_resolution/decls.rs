use trait_set::trait_set;

use crate::{interner::Tok, VName};

trait_set! {
  pub trait InjectedAsFn = Fn(&[Tok<String>]) -> Option<VName>;
  pub trait UpdatedFn = Fn(&[Tok<String>]) -> bool;
}
