use std::sync::Arc;

use itertools::Itertools;

use crate::api;
use crate::interner::{deintern, Tok};
use crate::location::Pos;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
#[derive(Clone, Debug)]
pub struct ErrPos {
  /// The suspected origin
  pub position: Pos,
  /// Any information about the role of this origin
  pub message: Option<Arc<String>>,
}
impl ErrPos {
  pub fn from_api(pel: &api::ErrLocation) -> Self {
    Self {
      message: Some(pel.message.clone()).filter(|s| !s.is_empty()),
      position: Pos::from_api(&pel.location),
    }
  }
  pub fn to_api(&self) -> api::ErrLocation {
    api::ErrLocation {
      message: self.message.clone().unwrap_or_default(),
      location: self.position.to_api(),
    }
  }
  pub fn new(msg: &str, position: Pos) -> Self {
    Self { message: Some(Arc::new(msg.to_string())), position }
  }
}
impl From<Pos> for ErrPos {
  fn from(origin: Pos) -> Self { Self { position: origin, message: None } }
}

#[derive(Clone, Debug)]
pub struct OrcErr {
  pub description: Tok<String>,
  pub message: Arc<String>,
  pub positions: Vec<ErrPos>,
}
impl OrcErr {
  pub fn from_api(err: &api::OrcError) -> Self {
    Self {
      description: deintern(err.description),
      message: err.message.clone(),
      positions: err.locations.iter().map(ErrPos::from_api).collect(),
    }
  }
  pub fn to_api(&self) -> api::OrcError {
    api::OrcError {
      description: self.description.marker(),
      message: self.message.clone(),
      locations: self.positions.iter().map(ErrPos::to_api).collect(),
    }
  }
}
impl Eq for OrcErr {}
impl PartialEq for OrcErr {
  fn eq(&self, other: &Self) -> bool { self.description == other.description }
}
impl From<OrcErr> for Vec<OrcErr> {
  fn from(value: OrcErr) -> Self { vec![value] }
}

pub fn errv_to_apiv<'a>(errv: impl IntoIterator<Item = &'a OrcErr>) -> Vec<api::OrcError> {
  errv.into_iter().map(OrcErr::to_api).collect_vec()
}

pub fn errv_from_apiv<'a>(err: impl IntoIterator<Item = &'a api::OrcError>) -> Vec<OrcErr> {
  err.into_iter().map(OrcErr::from_api).collect()
}

pub type OrcRes<T> = Result<T, Vec<OrcErr>>;

pub fn mk_err(
  description: Tok<String>,
  message: impl AsRef<str>,
  posv: impl IntoIterator<Item = ErrPos>,
) -> OrcErr {
  OrcErr {
    description,
    message: Arc::new(message.as_ref().to_string()),
    positions: posv.into_iter().collect(),
  }
}

pub trait Reporter {
  fn report(&self, e: OrcErr);
}
