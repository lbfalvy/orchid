use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;
use ordered_float::NotNan;

use crate::parser::CharFilter;
use crate::proto::{HostExtNotif, HostExtReq};

/// ID of a system type
pub type SysDeclId = u16;

/// ID of a system instance
pub type SysId = u16;

/// Details about a system provided by this library
#[derive(Debug, Clone, Coding)]
pub struct SystemDecl {
  /// ID of the system, unique within the library
  pub id: SysDeclId,
  /// This can be depended upon. Exactly one of each kind will be loaded
  pub name: String,
  /// If multiple instances of a system are found, the highest priority will be
  /// used. This can be used for version counting, but also for fallbacks if a
  /// negative number is found.
  ///
  /// Systems cannot depend on specific versions and older versions of systems
  /// are never loaded. Compatibility can be determined on a per-system basis
  /// through an algorithm chosen by the provider.
  pub priority: NotNan<f64>,
  /// List of systems needed for this one to work correctly. These will be
  /// looked up, and an error produced if they aren't found.
  pub depends: Vec<String>,
}

/// Host -> extension; instantiate a system according to its [SystemDecl].
/// Multiple instances of a system may exist in the same address space, so it's
/// essential that any resource associated with a system finds its system by the
/// ID in a global map.
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct NewSystem {
  /// ID of the system
  pub system: SysDeclId,
  /// ID of the system instance, unique for the host
  pub id: SysId,
  /// Instance IDs for dependencies, in the order that the names appear in the
  /// declaration
  pub depends: Vec<SysId>,
}
impl Request for NewSystem {
  type Response = SystemInst;
}

#[derive(Clone, Debug, Coding)]
pub struct SystemInst {
  /// The set of possible starting characters of tokens the lexer of this system
  /// can process. The lexer will notify this system if it encounters one of
  /// these characters.9
  pub lex_filter: CharFilter,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtNotif)]
pub struct SystemDrop(pub SysId);
