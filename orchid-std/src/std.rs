use orchid_extension::atom::owned_atom_info;
use orchid_extension::fs::DeclFs;
use orchid_extension::system::{System, SystemCard};
use orchid_extension::system_ctor::SystemCtor;
use orchid_extension::tree::GenTree;

use crate::string::str_atom::StringAtom;
use crate::string::str_leer::StringLexer;

#[derive(Default)]
pub struct StdSystem;
impl SystemCtor for StdSystem {
  type Deps = ();
  type Instance = Self;
  const NAME: &'static str = "orchid::std";
  const VERSION: f64 = 0.00_01;
  fn inst() -> Option<Self::Instance> { Some(StdSystem) }
}
impl SystemCard for StdSystem {
  type Ctor = Self;
  const ATOM_DEFS: &'static [Option<orchid_extension::atom::AtomInfo>] =
    &[Some(owned_atom_info::<StringAtom>())];
}
impl System for StdSystem {
  fn lexers() -> Vec<orchid_extension::lexer::LexerObj> { vec![&StringLexer] }
  fn vfs() -> DeclFs { DeclFs::Mod(&[]) }
  fn env() -> GenTree {
    GenTree::module([("std", GenTree::module([("string", GenTree::module([]))]))])
  }
}
