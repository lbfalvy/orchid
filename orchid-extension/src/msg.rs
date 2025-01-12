use std::pin::pin;

use async_std::io;
use orchid_base::msg::{recv_msg, send_msg};

pub async fn send_parent_msg(msg: &[u8]) -> io::Result<()> {
	send_msg(pin!(io::stdout()), msg).await
}
pub async fn recv_parent_msg() -> io::Result<Vec<u8>> { recv_msg(pin!(io::stdin())).await }
