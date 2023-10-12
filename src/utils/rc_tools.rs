use std::rc::Rc;
use std::sync::Arc;

pub fn rc_to_owned<T: Clone>(rc: Rc<T>) -> T {
  Rc::try_unwrap(rc).unwrap_or_else(|rc| rc.as_ref().clone())
}

pub fn arc_to_owned<T: Clone>(rc: Arc<T>) -> T {
  Arc::try_unwrap(rc).unwrap_or_else(|rc| rc.as_ref().clone())
}

pub fn map_rc<T: Clone, U>(rc: Rc<T>, pred: impl FnOnce(T) -> U) -> Rc<U> {
  Rc::new(pred(rc_to_owned(rc)))
}
