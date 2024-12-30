use crate::{api, rule::shared::Matcher, tree::Code};
use std::sync::{Arc, RwLock};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use lazy_static::lazy_static;
use orchid_base::{macros::{mtreev_from_api, mtreev_to_api, MTok, MTree}, name::Sym};
use ordered_float::NotNan;
use trait_set::trait_set;
use crate::extension::AtomHand;

pub type MacTok = MTok<'static, AtomHand>;
pub type MacTree = MTree<'static, AtomHand>;

trait_set!{
  trait MacroCB = Fn(Vec<MacTree>) -> Option<Vec<MacTree>> + Send + Sync;
}

lazy_static!{
  static ref RECURSION: RwLock<HashMap<api::ParsId, Box<dyn MacroCB>>> = RwLock::default();
  static ref MACRO_SLOTS: RwLock<HashMap<api::ParsId,
    HashMap<api::MacroTreeId, Arc<MacTok>>
  >> = RwLock::default();
}

pub fn macro_recur(run_id: api::ParsId, input: Vec<MacTree>) -> Option<Vec<MacTree>> {
  (RECURSION.read().unwrap()[&run_id])(input)
}

pub fn macro_treev_to_api(run_id: api::ParsId, mtree: Vec<MacTree>) -> Vec<api::MacroTree> {
  let mut g = MACRO_SLOTS.write().unwrap();
  let run_cache = g.get_mut(&run_id).expect("Parser run not found");
  mtreev_to_api(&mtree, &mut |a: &AtomHand| {
    let id = api::MacroTreeId((run_cache.len() as u64 + 1).try_into().unwrap());
    run_cache.insert(id, Arc::new(MacTok::Atom(a.clone())));
    api::MacroToken::Slot(id)
  })
}

pub fn macro_treev_from_api(api: Vec<api::MacroTree>) -> Vec<MacTree> {
  mtreev_from_api(&api, &mut |atom| MacTok::Atom(AtomHand::from_api(atom.clone())))
}

pub fn deslot_macro(run_id: api::ParsId, tree: &[MacTree]) -> Option<Vec<MacTree>> {
  let mut slots = (MACRO_SLOTS.write().unwrap())
    .remove(&run_id).expect("Run not found");
  return work(&mut slots, tree);
  fn work(
    slots: &mut HashMap<api::MacroTreeId, Arc<MacTok>>,
    tree: &[MacTree]
  ) -> Option<Vec<MacTree>> {
    let items = (tree.iter())
      .map(|t| Some(MacTree {
        tok: match &*t.tok {
          MacTok::Atom(_) | MacTok::Name(_) | MacTok::Ph(_) => return None,
          MacTok::Ref(_) => panic!("Ref is an extension-local optimization"),
          MacTok::Slot(slot) => slots.get(&slot.id()).expect("Slot not found").clone(),
          MacTok::S(paren, b) => Arc::new(MacTok::S(*paren, work(slots, b)?)),
          MacTok::Lambda(a, b) => Arc::new(match (work(slots, a), work(slots, b)) {
            (None, None) => return None,
            (Some(a), None) => MacTok::Lambda(a, b.clone()),
            (None, Some(b)) => MacTok::Lambda(a.clone(), b),
            (Some(a), Some(b)) => MacTok::Lambda(a, b),
          }),
        },
        pos: t.pos.clone()
      }))
      .collect_vec();
    let any_changed = items.iter().any(Option::is_some);
    any_changed.then(|| {
      (items.into_iter().enumerate())
        .map(|(i, opt)| opt.unwrap_or_else(|| tree[i].clone()))
        .collect_vec()
    })
  }
}

pub struct MacroRepo{
  no_prio: Vec<(HashSet<Sym>, Matcher, Code)>,
  prio: Vec<(HashSet<Sym>, NotNan<f64>, Matcher, Code)>,
}

pub fn match_on_exprv<'a>(target: &'a [MacTree], pattern: &[MacTree]) -> Option<MatchhState<'a>> {
  
}