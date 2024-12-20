use std::sync::Arc;

use hashbrown::HashMap;
use orchid_api::PhKind;
use orchid_base::tree::Ph;
use orchid_base::{interner::Tok, join::join_maps};
use orchid_base::location::Pos;

use crate::macros::{MacTok, MacTree};
use orchid_base::name::Sym;

#[derive(Clone, Copy, Debug)]
pub enum StateEntry<'a> {
  Vec(&'a [MacTree]),
  Scalar(&'a MacTree),
}
#[derive(Clone)]
pub struct MatchState<'a> {
  placeholders: HashMap<Tok<String>, StateEntry<'a>>,
  name_posv: HashMap<Sym, Vec<Pos>>,
}
impl<'a> MatchState<'a> {
  pub fn from_ph(key: Tok<String>, entry: StateEntry<'a>) -> Self {
    Self { placeholders: HashMap::from([(key, entry)]), name_posv: HashMap::new() }
  }
  pub fn combine(self, s: Self) -> Self {
    Self {
      placeholders: self.placeholders.into_iter().chain(s.placeholders).collect(),
      name_posv: join_maps(self.name_posv, s.name_posv, |_, l, r| {
        l.into_iter().chain(r).collect()
      }),
    }
  }
  pub fn ph_len(&self, key: &Tok<String>) -> Option<usize> {
    match self.placeholders.get(key)? {
      StateEntry::Vec(slc) => Some(slc.len()),
      _ => None,
    }
  }
  pub fn from_name(name: Sym, location: Pos) -> Self {
    Self { name_posv: HashMap::from([(name, vec![location])]), placeholders: HashMap::new() }
  }
}
impl Default for MatchState<'static> {
  fn default() -> Self { Self { name_posv: HashMap::new(), placeholders: HashMap::new() } }
}

#[must_use]
pub fn apply_exprv(template: &[MacTree], state: &MatchState) -> Vec<MacTree> {
  template.iter().map(|e| apply_expr(e, state)).flat_map(Vec::into_iter).collect()
}

#[must_use]
pub fn apply_expr(template: &MacTree, state: &MatchState) -> Vec<MacTree> {
  let MacTree { pos, tok } = template;
  match &**tok {
    MacTok::Name(n) => match state.name_posv.get(n) {
      None => vec![template.clone()],
      Some(locs) => vec![MacTree { tok: tok.clone(), pos: locs[0].clone() }],
    },
    MacTok::Atom(_) => vec![template.clone()],
    MacTok::S(c, body) => vec![MacTree {
      pos: pos.clone(), tok: Arc::new(MacTok::S(*c, apply_exprv(body.as_slice(), state))),
    }],
    MacTok::Ph(Ph { name, kind }) => {
      let Some(value) = state.placeholders.get(name) else {
        panic!("Placeholder does not have a value in state")
      };
      match (kind, value) {
        (PhKind::Scalar, StateEntry::Scalar(item)) => vec![(*item).clone()],
        (PhKind::Vector { .. }, StateEntry::Vec(chunk)) => chunk.to_vec(),
        _ => panic!("Type mismatch between template and state"),
      }
    },
    MacTok::Lambda(arg, body) => vec![MacTree {
      pos: pos.clone(),
      tok: Arc::new(MacTok::Lambda(
        apply_exprv(arg, state),
        apply_exprv(&body[..], state),
      )),
    }],
    MacTok::Slot(_) | MacTok::Ref(_) => panic!("Extension-only variants"),
  }
}
