use std::io;

use orchid_base::msg::{recv_msg, send_msg};

pub fn send_parent_msg(msg: &[u8]) -> io::Result<()> { send_msg(&mut io::stdout().lock(), msg) }
pub fn recv_parent_msg() -> io::Result<Vec<u8>> { recv_msg(&mut io::stdin().lock()) }
