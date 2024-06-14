use orchid_api_derive::Coding;

use crate::intern::TStr;
use crate::location::Location;

pub type ProjErrId = u16;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct ProjErrLocation {
  /// Description of the relation of this location to the error. If not used,
  /// set to empty string
  message: String,
  /// Location in code where the error emerged. This is usually [Location::Gen].
  location: Location,
}

/// Programming errors raised by extensions. At runtime these produce the
/// equivalent of a Haskell bottom. Note that runtime errors produced by
/// fallible operations should return an Orchid result and not a bottom.
/// For example, file reading produces result::err when the file doesn't exist,
/// and a bottom if the file name isn't a string.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct ProjErr {
  /// General description of the kind of error.
  description: TStr,
  /// Specific information about the exact error, preferably containing concrete
  /// values.
  message: String,
  /// Specific code fragments that have contributed to the emergence of the
  /// error.
  locations: Vec<ProjErrLocation>,
}

/// If this is an [`Err`] then the [`Vec`] must not be empty.
pub type ProjResult<T> = Result<T, Vec<ProjErr>>;
