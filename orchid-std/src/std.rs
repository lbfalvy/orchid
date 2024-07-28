use std::sync::Arc;

use orchid_base::interner::Tok;
use orchid_extension::atom::{AtomDynfo, AtomicFeatures};
use orchid_extension::fs::DeclFs;
use orchid_extension::fun::Fun;
use orchid_extension::system::{System, SystemCard};
use orchid_extension::system_ctor::SystemCtor;
use orchid_extension::tree::{cnst, module, root_mod, GenMemberKind};

use crate::number::num_atom::{Float, Int};
use crate::string::str_atom::StrAtom;
use crate::string::str_lexer::StringLexer;
use crate::OrcString;

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
  const ATOM_DEFS: &'static [Option<&'static dyn AtomDynfo>] =
    &[Some(Int::INFO), Some(Float::INFO), Some(StrAtom::INFO)];
}
impl System for StdSystem {
  fn lexers() -> Vec<orchid_extension::lexer::LexerObj> { vec![&StringLexer] }
  fn vfs() -> DeclFs { DeclFs::Mod(&[]) }
  fn env() -> Vec<(Tok<String>, GenMemberKind)> {
    vec![
      root_mod("std", [], [
        module(true, "string", [], [
          cnst(true, "concat", Fun::new(|left: OrcString| {
            Fun::new(move |right: OrcString| {
              StrAtom::new(Arc::new(left.get_string().to_string() + &right.get_string()))
            })
          }))
        ]),
      ])
    ]
  }
}
