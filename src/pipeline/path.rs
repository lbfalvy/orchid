use intern_all::{i, Tok};

use crate::error::{ProjectError, ProjectResult};
use crate::location::CodeLocation;
use crate::name::VName;

/// Turn a relative (import) path into an absolute path.
/// If the import path is empty, the return value is also empty.
///
/// # Errors
///
/// if the relative path contains as many or more `super` segments than the
/// length of the absolute path.
pub(super) fn absolute_path(
  abs_location: &[Tok<String>],
  rel_path: &[Tok<String>],
  location: CodeLocation,
) -> ProjectResult<VName> {
  match absolute_path_rec(abs_location, rel_path) {
    Some(v) => VName::new(v).map_err(|_| ImportAll { location }.pack()),
    None => {
      let path = rel_path.try_into().expect("At least one super");
      Err(TooManySupers { path, location }.pack())
    },
  }
}

#[must_use = "this could be None which means that there are too many supers"]
fn absolute_path_rec(
  mut abs_location: &[Tok<String>],
  mut rel_path: &[Tok<String>],
) -> Option<Vec<Tok<String>>> {
  let mut relative = false;
  if rel_path.first().cloned() == Some(i("self")) {
    relative = true;
    rel_path = rel_path.split_first().expect("checked above").1;
  } else {
    while rel_path.first().cloned() == Some(i("super")) {
      match abs_location.split_last() {
        Some((_, torso)) => abs_location = torso,
        None => return None,
      };
      rel_path = rel_path.split_first().expect("checked above").1;
      relative = true;
    }
  }
  match relative {
    true => Some(abs_location.iter().chain(rel_path).cloned().collect()),
    false => Some(rel_path.to_vec()),
  }
}

/// Error produced when an import path starts with more `super` segments
/// than the current module's absolute path
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TooManySupers {
  /// The offending import path
  pub path: VName,
  /// The faulty import statement
  pub location: CodeLocation,
}
impl ProjectError for TooManySupers {
  const DESCRIPTION: &'static str = "an import path starts with more \
  `super` segments than the current module's absolute path";
  fn message(&self) -> String {
    format!("path {} contains too many `super` steps.", self.path)
  }

  fn one_position(&self) -> CodeLocation { self.location.clone() }
}

/// Error produced for the statement `import *`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ImportAll {
  /// The file containing the offending import
  pub location: CodeLocation,
}
impl ProjectError for ImportAll {
  const DESCRIPTION: &'static str = "a top-level glob import was used";
  fn message(&self) -> String { format!("{} imports *", self.location) }
  fn one_position(&self) -> CodeLocation { self.location.clone() }
}
