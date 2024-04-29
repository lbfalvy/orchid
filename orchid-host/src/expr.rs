use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use orchid_api::expr::ExprTicket;

use crate::host::AtomHand;
use lazy_static::lazy_static;

#[derive(Clone, Debug)]
pub struct RtExpr {
  is_canonical: Arc<AtomicBool>,
  data: Arc<()>,
}
impl RtExpr {
  pub fn as_atom(&self) -> Option<AtomHand> { todo!() }
  pub fn strong_count(&self) -> usize { todo!() }
  pub fn id(&self) -> u64 { self.data.as_ref() as *const () as usize as u64 }
  pub fn canonicalize(&self) -> ExprTicket {
    if !self.is_canonical.swap(true, Ordering::Relaxed) {
      KNOWN_EXPRS.write().unwrap().entry(self.id()).or_insert_with(|| self.clone());
    }
    self.id()
  }
  pub fn resolve(tk: ExprTicket) -> Option<Self> { KNOWN_EXPRS.read().unwrap().get(&tk).cloned() }
}
impl Drop for RtExpr {
  fn drop(&mut self) {
    // If the only two references left are this and known, remove from known
    if Arc::strong_count(&self.data) == 2 && self.is_canonical.load(Ordering::Relaxed) {
      // if known is poisoned, a leak is preferable to a panicking destructor
      if let Ok(mut w) = KNOWN_EXPRS.write() {
        w.remove(&self.id());
      }
    }
  }
}

lazy_static!{
  static ref KNOWN_EXPRS: RwLock<HashMap<ExprTicket, RtExpr>> = RwLock::default();
}
