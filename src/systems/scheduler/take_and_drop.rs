use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

use super::{SeqScheduler, SharedHandle};
use crate::foreign::cps_box::{init_cps, CPSBox};
use crate::foreign::Atom;
use crate::interpreted::Clause;
use crate::systems::AssertionError;
use crate::{define_fn, Primitive};

pub fn request<T: 'static>(
  handle: &SharedHandle<T>,
  request: Box<dyn Any>,
) -> Option<Box<dyn Any>> {
  if request.downcast::<TakerRequest>().is_ok() {
    let handle = handle.clone();
    let cmd = TakeCmd(Rc::new(move |sch| {
      let _ = sch.seal(handle.clone(), |_| Vec::new());
    }));
    return Some(Box::new(init_cps(1, cmd)))
  }
  None
}

pub struct TakerRequest;
#[derive(Clone)]
pub struct TakeCmd(pub Rc<dyn Fn(SeqScheduler)>);
impl Debug for TakeCmd {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "A command to drop a shared resource")
  }
}
define_fn! {
  pub TakeAndDrop = |x| x.inspect(|c| match c {
    Clause::P(Primitive::Atom(Atom(atomic))) => {
      let t = atomic.request(Box::new(TakerRequest))
        .ok_or_else(|| AssertionError::ext(x.clone(), "a SharedHandle"))?;
      let data: CPSBox<TakeCmd> = *t.downcast().expect("implied by request");
      Ok(data.atom_cls())
    },
    _ => AssertionError::fail(x.clone(), "an atom"),
  })
}
