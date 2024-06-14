use std::any::Any;
use std::io::{Read, Write};

use orchid_api::expr::ExprTicket;
use orchid_api::tree::TreeModule;
use typeid::ConstTypeId;

use crate::atom::AtomInfo;
use crate::expr::GenClause;
use crate::fs::DeclFs;
use crate::lexer::LexerObj;

/// System as consumed by foreign code
pub trait SystemCard: Default + Send + Sync + 'static {
  const NAME: &'static str;
  const ATOM_DEFS: &'static [Option<AtomInfo>];
}

pub trait DynSystemCard: Send + Sync + 'static {
  fn name(&self) -> &'static str;
  fn atoms(&self) -> &'static [Option<AtomInfo>];
  fn atom_info_for(&self, tid: ConstTypeId) -> Option<(usize, &AtomInfo)> {
    (self.atoms().iter().enumerate())
      .filter_map(|(i, o)| o.as_ref().map(|a| (i, a)))
      .find(|ent| ent.1.tid == tid)
  }
}

impl<T: SystemCard> DynSystemCard for T {
  fn name(&self) -> &'static str { Self::NAME }
  fn atoms(&self) -> &'static [Option<AtomInfo>] { Self::ATOM_DEFS }
}

/// System as defined by author
pub trait System: Send + Sync + SystemCard {
  fn env() -> TreeModule;
  fn source() -> DeclFs;
  const LEXERS: &'static [LexerObj];
}

pub trait DynSystem: Send + Sync + 'static {
  fn env(&self) -> TreeModule;
  fn source(&self) -> DeclFs;
  fn lexers(&self) -> &'static [LexerObj];
  fn card(&self) -> &dyn DynSystemCard;
}

impl<T: System> DynSystem for T {
  fn env(&self) -> TreeModule { <Self as System>::env() }
  fn source(&self) -> DeclFs { <Self as System>::source() }
  fn lexers(&self) -> &'static [LexerObj] { Self::LEXERS }
  fn card(&self) -> &dyn DynSystemCard { self }
}
