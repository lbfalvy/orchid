use std::sync::{Arc, RwLock};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use lazy_static::lazy_static;
use orchid_base::macros::{MTok, MTree, mtreev_from_api, mtreev_to_api};
use orchid_base::name::Sym;
use trait_set::trait_set;

use crate::api;
use crate::extension::AtomHand;
use crate::rule::matcher::{NamedMatcher, PriodMatcher};
use crate::rule::state::{MatchState, OwnedState};
use crate::tree::Code;

pub type MacTok = MTok<'static, AtomHand>;
pub type MacTree = MTree<'static, AtomHand>;

trait_set! {
  trait MacroCB = Fn(Vec<MacTree>) -> Option<Vec<MacTree>> + Send + Sync;
}

lazy_static! {
  static ref RECURSION: RwLock<HashMap<api::ParsId, Box<dyn MacroCB>>> = RwLock::default();
  static ref MACRO_SLOTS: RwLock<HashMap<api::ParsId, HashMap<api::MacroTreeId, Arc<MacTok>>>> =
    RwLock::default();
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
  let mut slots = (MACRO_SLOTS.write().unwrap()).remove(&run_id).expect("Run not found");
  return work(&mut slots, tree);
  fn work(
    slots: &mut HashMap<api::MacroTreeId, Arc<MacTok>>,
    tree: &[MacTree],
  ) -> Option<Vec<MacTree>> {
    let items = (tree.iter())
      .map(|t| {
        Some(MacTree {
          tok: match &*t.tok {
            MacTok::Atom(_) | MacTok::Name(_) | MacTok::Ph(_) => return None,
            MacTok::Ref(_) => panic!("Ref is an extension-local optimization"),
            MacTok::Done(_) => panic!("Created and removed by matcher"),
            MacTok::Slot(slot) => slots.get(&slot.id()).expect("Slot not found").clone(),
            MacTok::S(paren, b) => Arc::new(MacTok::S(*paren, work(slots, b)?)),
            MacTok::Lambda(a, b) => Arc::new(match (work(slots, a), work(slots, b)) {
              (None, None) => return None,
              (Some(a), None) => MacTok::Lambda(a, b.clone()),
              (None, Some(b)) => MacTok::Lambda(a.clone(), b),
              (Some(a), Some(b)) => MacTok::Lambda(a, b),
            }),
          },
          pos: t.pos.clone(),
        })
      })
      .collect_vec();
    let any_changed = items.iter().any(Option::is_some);
    any_changed.then(|| {
      (items.into_iter().enumerate())
        .map(|(i, opt)| opt.unwrap_or_else(|| tree[i].clone()))
        .collect_vec()
    })
  }
}

pub struct Macro<Matcher> {
  deps: HashSet<Sym>,
  cases: Vec<(Matcher, Code)>,
}

pub struct MacroRepo {
  named: HashMap<Sym, Vec<Macro<NamedMatcher>>>,
  prio: Vec<Macro<PriodMatcher>>,
}
impl MacroRepo {
  /// TODO: the recursion inside this function needs to be moved into Orchid.
  /// See the markdown note
  pub fn process_exprv(&self, target: &[MacTree]) -> Option<Vec<MacTree>> {
    let mut workcp = target.to_vec();
    let mut lexicon;

    'try_named: loop {
      lexicon = HashSet::new();
      target.iter().for_each(|tgt| fill_lexicon(tgt, &mut lexicon));

      for (i, tree) in workcp.iter().enumerate() {
        let MacTok::Name(name) = &*tree.tok else { continue };
        let matches = (self.named.get(name).into_iter().flatten())
          .filter(|m| m.deps.is_subset(&lexicon))
          .filter_map(|mac| {
            mac.cases.iter().find_map(|cas| cas.0.apply(&workcp[i..], |_| false).map(|s| (cas, s)))
          })
          .collect_vec();
        assert!(
          matches.len() < 2,
          "Multiple conflicting matches on {:?}: {:?}",
          &workcp[i..],
          matches
        );
        let Some((case, (state, tail))) = matches.into_iter().next() else { continue };
        let inj = (run_body(&case.1, state).into_iter())
          .map(|MacTree { pos, tok }| MacTree { pos, tok: Arc::new(MacTok::Done(tok)) });
        workcp.splice(i..(workcp.len() - tail.len()), inj);
        continue 'try_named;
      }
      break;
    }

    if let Some(((_, body), state)) = (self.prio.iter())
      .filter(|mac| mac.deps.is_subset(&lexicon))
      .flat_map(|mac| &mac.cases)
      .find_map(|case| case.0.apply(&workcp, |_| false).map(|state| (case, state)))
    {
      return Some(run_body(body, state));
    }

    let results = (workcp.into_iter())
      .map(|mt| match &*mt.tok {
        MTok::S(p, body) => self.process_exprv(body).map(|body| MTok::S(*p, body).at(mt.pos)),
        MTok::Lambda(arg, body) => match (self.process_exprv(arg), self.process_exprv(body)) {
          (Some(arg), Some(body)) => Some(MTok::Lambda(arg, body).at(mt.pos)),
          (Some(arg), None) => Some(MTok::Lambda(arg, body.to_vec()).at(mt.pos)),
          (None, Some(body)) => Some(MTok::Lambda(arg.to_vec(), body).at(mt.pos)),
          (None, None) => None,
        },
        _ => None,
      })
      .collect_vec();
    results.iter().any(Option::is_some).then(|| {
      (results.into_iter().zip(target))
        .map(|(opt, fb)| opt.unwrap_or_else(|| fb.clone()))
        .collect_vec()
    })
  }
}

fn fill_lexicon(tgt: &MacTree, lexicon: &mut HashSet<Sym>) {
  match &*tgt.tok {
    MTok::Name(n) => {
      lexicon.insert(n.clone());
    },
    MTok::Lambda(arg, body) => {
      arg.iter().for_each(|t| fill_lexicon(t, lexicon));
      body.iter().for_each(|t| fill_lexicon(t, lexicon))
    },
    MTok::S(_, body) => body.iter().for_each(|t| fill_lexicon(t, lexicon)),
    _ => (),
  }
}

fn run_body(body: &Code, mut state: MatchState<'_>) -> Vec<MacTree> {
  let inject: Vec<MacTree> = todo!("Call the interpreter with bindings");
  inject
    .into_iter()
    .map(|MTree { pos, tok }| MTree { pos, tok: Arc::new(MTok::Done(tok)) })
    .collect_vec()
}
