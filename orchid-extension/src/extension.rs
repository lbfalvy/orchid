use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader};
use orchid_api_traits::{Decode, Encode};
use ordered_float::NotNan;

use crate::child::{recv_parent_msg, send_parent_msg};
use crate::clone;
use crate::intern::{init_replica, sweep_replica};
use crate::reqnot::{ReqNot, Requester as _};

pub struct SystemParams {
  deps: Vec<SystemHandle>,
  
}

pub struct SystemCtor {
  deps: Vec<String>,
  make: Box<dyn FnMut(SystemParams) -> System>,
  name: String,
  prio: NotNan<f64>,
  dependencies: Vec<String>,
}

pub struct ExtensionData {
  systems: Vec<SystemCtor>
}


