use std::collections::HashMap;
use std::rc::Rc;

use super::preparse::Preparsed;
use crate::representations::VName;

#[derive(Debug)]
pub struct LoadedSource {
  pub text: Rc<String>,
  pub preparsed: Preparsed,
}

pub type LoadedSourceTable = HashMap<VName, LoadedSource>;
