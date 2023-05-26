use trait_set::trait_set;

use crate::interner::{Sym, Tok};

trait_set! {
  pub trait InjectedAsFn = Fn(&[Tok<String>]) -> Option<Sym>;
}
