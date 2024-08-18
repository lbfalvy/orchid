use std::any::TypeId;
use std::num::NonZero;
use std::sync::Arc;

use hashbrown::HashMap;
use orchid_api::AtomId;
use orchid_api_traits::Decode;
use orchid_base::interner::Tok;
use orchid_base::logging::Logger;
use orchid_base::reqnot::ReqNot;

use crate::api;
use crate::atom::{get_info, AtomCtx, AtomDynfo, AtomicFeatures, ForeignAtom, TypAtom};
use crate::fs::DeclFs;
// use crate::fun::Fun;
use crate::lexer::LexerObj;
use crate::parser::ParserObj;
use crate::system_ctor::{CtedObj, SystemCtor};
use crate::tree::GenMemberKind;

/// System as consumed by foreign code
pub trait SystemCard: Default + Send + Sync + 'static {
  type Ctor: SystemCtor;
  const ATOM_DEFS: &'static [Option<&'static dyn AtomDynfo>];
}

pub trait DynSystemCard: Send + Sync + 'static {
  fn name(&self) -> &'static str;
  /// Atoms explicitly defined by the system card. Do not rely on this for
  /// querying atoms as it doesn't include the general atom types
  fn atoms(&self) -> &'static [Option<&'static dyn AtomDynfo>];
}

/// Atoms supported by this package which may appear in all extensions.
/// The indices of these are bitwise negated, such that the MSB of an atom index
/// marks whether it belongs to this package (0) or the importer (1)
fn general_atoms() -> &'static [Option<&'static dyn AtomDynfo>] { &[/*Some(Fun::INFO)*/] }

pub fn atom_info_for(
  sys: &(impl DynSystemCard + ?Sized),
  tid: TypeId,
) -> Option<(api::AtomId, &'static dyn AtomDynfo)> {
  (sys.atoms().iter().enumerate().map(|(i, o)| (NonZero::new(i as u64 + 1).unwrap(), o)))
    .chain(general_atoms().iter().enumerate().map(|(i, o)| (NonZero::new(!(i as u64)).unwrap(), o)))
    .filter_map(|(i, o)| o.as_ref().map(|a| (api::AtomId(i), *a)))
    .find(|ent| ent.1.tid() == tid)
}

pub fn atom_by_idx(
  sys: &(impl DynSystemCard + ?Sized),
  tid: api::AtomId,
) -> Option<&'static dyn AtomDynfo> {
  if (u64::from(tid.0) >> (u64::BITS - 1)) & 1 == 1 {
    general_atoms()[!u64::from(tid.0) as usize]
  } else {
    sys.atoms()[u64::from(tid.0) as usize - 1]
  }
}

pub fn resolv_atom(
  sys: &(impl DynSystemCard + ?Sized),
  atom: &api::Atom,
) -> &'static dyn AtomDynfo {
  let tid = api::AtomId::decode(&mut &atom.data[..8]);
  atom_by_idx(sys, tid).expect("Value of nonexistent type found")
}

impl<T: SystemCard> DynSystemCard for T {
  fn name(&self) -> &'static str { T::Ctor::NAME }
  fn atoms(&self) -> &'static [Option<&'static dyn AtomDynfo>] { Self::ATOM_DEFS }
}

/// System as defined by author
pub trait System: Send + Sync + SystemCard + 'static {
  fn env() -> Vec<(Tok<String>, GenMemberKind)>;
  fn vfs() -> DeclFs;
  fn lexers() -> Vec<LexerObj>;
  fn parsers() -> Vec<ParserObj>;
}

pub trait DynSystem: Send + Sync + DynSystemCard + 'static {
  fn dyn_env(&self) -> HashMap<Tok<String>, GenMemberKind>;
  fn dyn_vfs(&self) -> DeclFs;
  fn dyn_lexers(&self) -> Vec<LexerObj>;
  fn dyn_parsers(&self) -> Vec<ParserObj>;
  fn card(&self) -> &dyn DynSystemCard;
}

impl<T: System> DynSystem for T {
  fn dyn_env(&self) -> HashMap<Tok<String>, GenMemberKind> { Self::env().into_iter().collect() }
  fn dyn_vfs(&self) -> DeclFs { Self::vfs() }
  fn dyn_lexers(&self) -> Vec<LexerObj> { Self::lexers() }
  fn dyn_parsers(&self) -> Vec<ParserObj> { Self::parsers() }
  fn card(&self) -> &dyn DynSystemCard { self }
}

pub fn downcast_atom<A: AtomicFeatures>(foreign: ForeignAtom) -> Result<TypAtom<A>, ForeignAtom> {
  let mut data = &foreign.atom.data[..];
  let ctx = foreign.ctx.clone();
  let info_ent = (ctx.cted.deps().find(|s| s.id() == foreign.atom.owner))
    .map(|sys| get_info::<A>(sys.get_card()))
    .filter(|(pos, _)| AtomId::decode(&mut data) == *pos);
  match info_ent {
    None => Err(foreign),
    Some((_, info)) => {
      let val = info.decode(AtomCtx(data, foreign.atom.drop, ctx));
      let value = *val.downcast::<A::Data>().expect("atom decode returned wrong type");
      Ok(TypAtom { value, data: foreign })
    },
  }
}

#[derive(Clone)]
pub struct SysCtx {
  pub reqnot: ReqNot<api::ExtMsgSet>,
  pub id: api::SysId,
  pub cted: CtedObj,
  pub logger: Arc<Logger>,
}
impl SysCtx {
  pub fn new(
    id: api::SysId,
    cted: &CtedObj,
    logger: &Arc<Logger>,
    reqnot: ReqNot<api::ExtMsgSet>,
  ) -> Self {
    Self { cted: cted.clone(), id, logger: logger.clone(), reqnot }
  }
}
