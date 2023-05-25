use crate::interner::{Sym, Tok};

pub trait InjectedAsFn = Fn(&[Tok<String>]) -> Option<Sym>;
