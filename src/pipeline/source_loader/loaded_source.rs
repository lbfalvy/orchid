use std::collections::HashMap;
use std::rc::Rc;

use crate::representations::VName;

#[derive(Debug)]
pub struct LoadedSource {
  pub text: Rc<String>,
}

pub type LoadedSourceTable = HashMap<VName, LoadedSource>;
