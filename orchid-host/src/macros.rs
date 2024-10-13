use std::sync::RwLock;

use hashbrown::HashMap;
use lazy_static::lazy_static;
use orchid_base::macros::MTree;
use trait_set::trait_set;
use crate::api::ParsId;

trait_set!{
  trait MacroCB = Fn(Vec<MTree>) -> Option<Vec<MTree>> + Send + Sync;
}

lazy_static!{
  static ref RECURSION: RwLock<HashMap<ParsId, Box<dyn MacroCB>>> = RwLock::default();
}

pub fn macro_recur(run_id: ParsId, input: Vec<MTree>) -> Option<Vec<MTree>> {
  (RECURSION.read().unwrap()[&run_id])(input)
}

