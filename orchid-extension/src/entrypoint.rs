use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader};
use orchid_api_traits::{Decode, Encode};
use orchid_base::clone;
use orchid_base::intern::{init_replica, sweep_replica};
use orchid_base::reqnot::{ReqNot, Requester};

use crate::data::ExtensionData;
use crate::msg::{recv_parent_msg, send_parent_msg};

pub fn main(data: ExtensionData) {
  HostHeader::decode(&mut &recv_parent_msg().unwrap()[..]);
  let mut buf = Vec::new();
  let decls = data.systems.iter().map(|sys| sys.decl()).collect_vec();
  let systems = Arc::new(Mutex::new(HashMap::new()));
  ExtensionHeader { systems: decls.clone() }.encode(&mut buf);
  send_parent_msg(&buf).unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let rn = ReqNot::<ExtMsgSet>::new(
    |a, _| send_parent_msg(a).unwrap(),
    clone!(exiting; move |n, _| match n {
      HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      _ => todo!(),
    }),
    clone!(systems; move |req| match req.req() {
      HostExtReq::Ping(ping) => req.handle(ping, &()),
      HostExtReq::Sweep(sweep) => req.handle(sweep, &sweep_replica()),
      HostExtReq::NewSystem(new_sys) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let system = data.systems[i].new_system(new_sys, req.reqnot());
        systems.lock().unwrap().insert(new_sys.id, system);
        req.handle(new_sys, &())
      },
      _ => todo!(),
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    rn.receive(recv_parent_msg().unwrap())
  }
}
