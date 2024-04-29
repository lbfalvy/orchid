use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader};
use orchid_api_traits::{Decode, Encode};
use orchid_base::child::{recv_parent_msg, send_parent_msg};
use orchid_base::clone;
use orchid_base::intern::{init_replica, sweep_replica};
use orchid_base::reqnot::{ReqNot, Requester};

pub struct ExtensionData {}

pub fn main(data: &mut ExtensionData) {
  HostHeader::decode(&mut &recv_parent_msg().unwrap()[..]);
  let mut buf = Vec::new();
  ExtensionHeader { systems: vec![] }.encode(&mut buf);
  send_parent_msg(&buf).unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let rn = ReqNot::<ExtMsgSet>::new(
    |a, _| send_parent_msg(a).unwrap(),
    clone!(exiting; move |n, _| match n {
      HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      _ => todo!(),
    }),
    |req| match req.req() {
      HostExtReq::Ping(ping) => req.handle(ping, &()),
      HostExtReq::Sweep(sweep) => req.handle(sweep, &sweep_replica()),
      _ => todo!(),
    },
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    rn.receive(recv_parent_msg().unwrap())
  }
}
