use std::{rc::Rc, collections::HashMap};

use crate::interner::Token;

use super::preparse::Preparsed;

#[derive(Debug)]
pub struct LoadedSource {
  pub text: Rc<String>,
  pub preparsed: Preparsed,
}

pub type LoadedSourceTable = HashMap<Token<Vec<Token<String>>>, LoadedSource>;