use crate::interner::Token;

pub trait InjectedAsFn = Fn(
  &[Token<String>]
) -> Option<Token<Vec<Token<String>>>>;