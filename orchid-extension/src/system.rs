use orchid_api::proto::ExtMsgSet;
use orchid_api::system::SysId;
use orchid_base::reqnot::ReqNot;
use typeid::ConstTypeId;

use crate::atom::{decode_atom, owned_atom_info, AtomCard, AtomInfo, ForeignAtom, TypAtom};
use crate::fs::DeclFs;
use crate::fun::Fun;
use crate::lexer::LexerObj;
use crate::system_ctor::{CtedObj, SystemCtor};
use crate::tree::GenTree;

/// System as consumed by foreign code
pub trait SystemCard: Default + Send + Sync + 'static {
  type Ctor: SystemCtor;
  const ATOM_DEFS: &'static [Option<AtomInfo>];
}

pub trait DynSystemCard: Send + Sync + 'static {
  fn name(&self) -> &'static str;
  /// Atoms explicitly defined by the system card. Do not rely on this for
  /// querying atoms as it doesn't include the general atom types
  fn atoms(&self) -> &'static [Option<AtomInfo>];
}

/// Atoms supported by this package which may appear in all extensions.
/// The indices of these are bitwise negated, such that the MSB of an atom index
/// marks whether it belongs to this package (0) or the importer (1)
const GENERAL_ATOMS: &[Option<AtomInfo>] = &[Some(owned_atom_info::<Fun>())];

pub fn atom_info_for(
  sys: &(impl DynSystemCard + ?Sized),
  tid: ConstTypeId,
) -> Option<(u64, &AtomInfo)> {
  (sys.atoms().iter().enumerate().map(|(i, o)| (i as u64, o)))
    .chain(GENERAL_ATOMS.iter().enumerate().map(|(i, o)| (!(i as u64), o)))
    .filter_map(|(i, o)| o.as_ref().map(|a| (i, a)))
    .find(|ent| ent.1.tid == tid)
}

pub fn atom_by_idx(sys: &(impl DynSystemCard + ?Sized), tid: u64) -> Option<&AtomInfo> {
  if (tid >> (u64::BITS - 1)) & 1 == 1 {
    GENERAL_ATOMS[!tid as usize].as_ref()
  } else {
    sys.atoms()[tid as usize].as_ref()
  }
}

impl<T: SystemCard> DynSystemCard for T {
  fn name(&self) -> &'static str { T::Ctor::NAME }
  fn atoms(&self) -> &'static [Option<AtomInfo>] { Self::ATOM_DEFS }
}

/// System as defined by author
pub trait System: Send + Sync + SystemCard + 'static {
  fn env() -> GenTree;
  fn vfs() -> DeclFs;
  fn lexers() -> Vec<LexerObj>;
}

pub trait DynSystem: Send + Sync + 'static {
  fn dyn_env(&self) -> GenTree;
  fn dyn_vfs(&self) -> DeclFs;
  fn dyn_lexers(&self) -> Vec<LexerObj>;
  fn dyn_card(&self) -> &dyn DynSystemCard;
}

impl<T: System> DynSystem for T {
  fn dyn_env(&self) -> GenTree { Self::env() }
  fn dyn_vfs(&self) -> DeclFs { Self::vfs() }
  fn dyn_lexers(&self) -> Vec<LexerObj> { Self::lexers() }
  fn dyn_card(&self) -> &dyn DynSystemCard { self }
}

pub fn downcast_atom<A: AtomCard>(foreign: ForeignAtom) -> Result<TypAtom<A>, ForeignAtom> {
  match (foreign.expr.get_ctx().cted.deps())
    .find(|s| s.id() == foreign.atom.owner)
    .and_then(|sys| decode_atom::<A>(sys.get_card(), &foreign.atom))
  {
    None => Err(foreign),
    Some(value) => Ok(TypAtom { value, data: foreign }),
  }
}

#[derive(Clone)]
pub struct SysCtx {
  pub reqnot: ReqNot<ExtMsgSet>,
  pub id: SysId,
  pub cted: CtedObj,
}
