use crate::foreign::cps_box::init_cps;
use crate::foreign::InertAtomic;
use crate::systems::asynch::MessagePort;
use crate::systems::scheduler::SeqScheduler;
use crate::{define_fn, OrcString};

#[derive(Debug, Clone)]
struct ReadFile(OrcString);
impl InertAtomic for ReadFile {
  fn type_str() -> &'static str { "a readfile command" }
}

pub fn read_file(port: MessagePort, cmd: ReadFile) -> Vec<ExprInst> {
  let new_file = 
}

define_fn! {
  pub OpenFileRead = |x| Ok(init_cps(3, ReadFile(x.downcast()?)))
}
