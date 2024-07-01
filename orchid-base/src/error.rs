use std::sync::Arc;

use orchid_api::error::{ProjErr, ProjErrLocation};

use crate::interner::{deintern, Tok};
use crate::location::Pos;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
#[derive(Clone)]
pub struct ErrorPosition {
  /// The suspected origin
  pub position: Pos,
  /// Any information about the role of this origin
  pub message: Option<Arc<String>>,
}
impl ErrorPosition {
  pub fn from_api(pel: &ProjErrLocation) -> Self {
    Self {
      message: Some(pel.message.clone()).filter(|s| !s.is_empty()),
      position: Pos::from_api(&pel.location),
    }
  }
  pub fn to_api(&self) -> ProjErrLocation {
    ProjErrLocation {
      message: self.message.clone().unwrap_or_default(),
      location: self.position.to_api(),
    }
  }
}
impl From<Pos> for ErrorPosition {
  fn from(origin: Pos) -> Self { Self { position: origin, message: None } }
}

pub struct ErrorDetails {
  pub description: Tok<String>,
  pub message: Arc<String>,
  pub locations: Vec<ErrorPosition>,
}
impl ErrorDetails {
  pub fn from_api(err: &ProjErr) -> Self {
    Self {
      description: deintern(err.description),
      message: err.message.clone(),
      locations: err.locations.iter().map(ErrorPosition::from_api).collect(),
    }
  }
}
