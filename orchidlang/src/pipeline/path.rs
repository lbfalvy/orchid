use intern_all::{i, Tok};

use crate::error::{ErrorSansOrigin, ResultSansOrigin};
use crate::name::VName;

/// Turn a relative (import) path into an absolute path.
/// If the import path is empty, the return value is also empty.
///
/// # Errors
///
/// if the relative path contains as many or more `super` segments than the
/// length of the absolute path.
pub(super) fn absolute_path(cwd: &[Tok<String>], rel: &[Tok<String>]) -> ResultSansOrigin<VName> {
  absolute_path_rec(cwd, rel)
    .ok_or_else(|| TooManySupers { path: rel.try_into().expect("At least one super") }.pack())
    .and_then(|v| VName::new(v).map_err(|_| ImportAll.pack()))
}

#[must_use = "this could be None which means that there are too many supers"]
fn absolute_path_rec(mut cwd: &[Tok<String>], mut rel: &[Tok<String>]) -> Option<Vec<Tok<String>>> {
  let mut relative = false;
  if rel.first().cloned() == Some(i!(str: "self")) {
    relative = true;
    rel = rel.split_first().expect("checked above").1;
  } else {
    while rel.first().cloned() == Some(i!(str: "super")) {
      match cwd.split_last() {
        Some((_, torso)) => cwd = torso,
        None => return None,
      };
      rel = rel.split_first().expect("checked above").1;
      relative = true;
    }
  }
  match relative {
    true => Some(cwd.iter().chain(rel).cloned().collect()),
    false => Some(rel.to_vec()),
  }
}

/// Error produced when an import path starts with more `super` segments
/// than the current module's absolute path
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TooManySupers {
  /// The offending import path
  pub path: VName,
}
impl ErrorSansOrigin for TooManySupers {
  const DESCRIPTION: &'static str = "an import path starts with more \
  `super` segments than the current module's absolute path";
  fn message(&self) -> String { format!("path {} contains too many `super` steps.", self.path) }
}

/// Error produced for the statement `import *`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ImportAll;
impl ErrorSansOrigin for ImportAll {
  const DESCRIPTION: &'static str = "`import *` is forbidden";
}
