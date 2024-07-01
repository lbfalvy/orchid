use std::num::NonZeroU16;
use std::sync::Arc;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::interner::TStr;
use crate::location::Location;
use crate::proto::{ExtHostNotif, ExtHostReq};
use crate::system::SysId;

pub type ProjErrId = NonZeroU16;

#[derive(Clone, Debug, Coding)]
pub struct ProjErrLocation {
  /// Description of the relation of this location to the error. If not used,
  /// set to empty string
  pub message: Arc<String>,
  /// Location in code where the error emerged. This is usually [Location::Gen].
  pub location: Location,
}

/// Programming errors raised by extensions. At runtime these produce the
/// equivalent of a Haskell bottom. Note that runtime errors produced by
/// fallible operations should return an Orchid result and not a bottom.
/// For example, file reading produces result::err when the file doesn't exist,
/// and a bottom if the file name isn't a string.
#[derive(Clone, Debug, Coding)]
pub struct ProjErr {
  /// General description of the kind of error.
  pub description: TStr,
  /// Specific information about the exact error, preferably containing concrete
  /// values.
  pub message: Arc<String>,
  /// Specific code fragments that have contributed to the emergence of the
  /// error.
  pub locations: Vec<ProjErrLocation>,
}

/// When the interpreter encounters an error while serving a system's request,
/// it sends this error as an ID back to the system to save bandwidth.
/// The lifetime of this ID is the request being served, the receiving system
/// can return it and query its details with [GetDetails].
#[derive(Clone, Debug, Coding)]
pub enum ProjErrOrRef {
  New(ProjErr),
  Known(ProjErrId),
}

/// If this is an [`Err`] then the [`Vec`] must not be empty.
pub type ProjResult<T> = Result<T, Vec<ProjErrOrRef>>;

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ProjErrReq, ExtHostReq)]
pub struct GetErrorDetails(pub ProjErrId);
impl Request for GetErrorDetails {
  type Response = ProjErr;
}

/// Notify the host about an error without being forced to return said error.
/// This will still count as an error, but later operations that depend on the
/// value returned by the currently running function will get to run
///
/// The error is not connected to the place it was raised, since multiple calls
/// can be issued to a system at the same time
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ErrNotif, ExtHostNotif)]
pub struct ReportError(pub SysId, pub ProjErrOrRef);

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
#[extendable]
pub enum ProjErrReq {
  GetErrorDetails(GetErrorDetails),
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostNotif)]
#[extendable]
pub enum ErrNotif {
  ReportError(ReportError),
}
