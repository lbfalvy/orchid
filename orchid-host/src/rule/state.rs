#![allow(unused)]
use std::any::Any;

use hashbrown::HashMap;
use orchid_base::interner::Tok;
use orchid_base::join::join_maps;
use orchid_base::location::Pos;
use orchid_base::match_mapping;
use orchid_base::name::Sym;

use crate::macros::MacTree;

enum StackAction {
	Return(Box<dyn Any>),
	Call {
		target: Box<dyn FnOnce(Box<dyn Any>) -> StackAction>,
		param: Box<dyn Any>,
		tail: Box<dyn FnOnce(Box<dyn Any>) -> StackAction>,
	},
}

struct Trampoline {
	stack: Vec<Box<dyn FnOnce(Box<dyn Any>) -> StackAction>>,
}

#[derive(Clone, Copy, Debug)]
pub enum StateEntry<'a> {
	Vec(&'a [MacTree]),
	Scalar(&'a MacTree),
}
#[derive(Clone, Debug)]
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
			name_posv: join_maps(self.name_posv, s.name_posv, |_, l, r| l.into_iter().chain(r).collect()),
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
	pub fn remove(&mut self, name: Tok<String>) -> Option<StateEntry<'a>> {
		self.placeholders.remove(&name)
	}
	pub fn mk_owned(self) -> OwnedState {
		OwnedState {
			placeholders: (self.placeholders.into_iter())
				.map(|(k, v)| {
					(
						k.clone(),
						match_mapping!(v, StateEntry => OwnedEntry {
							Scalar(tree.clone()),
							Vec(v.to_vec()),
						}),
					)
				})
				.collect(),
			name_posv: self.name_posv,
		}
	}
}
impl Default for MatchState<'static> {
	fn default() -> Self { Self { name_posv: HashMap::new(), placeholders: HashMap::new() } }
}

#[derive(Clone, Debug)]
pub enum OwnedEntry {
	Vec(Vec<MacTree>),
	Scalar(MacTree),
}
pub struct OwnedState {
	placeholders: HashMap<Tok<String>, OwnedEntry>,
	name_posv: HashMap<Sym, Vec<Pos>>,
}
impl OwnedState {
	pub fn get(&self, key: &Tok<String>) -> Option<&OwnedEntry> { self.placeholders.get(key) }
	pub fn positions(&self, name: &Sym) -> &[Pos] { self.name_posv.get(name).map_or(&[], |v| &v[..]) }
}
