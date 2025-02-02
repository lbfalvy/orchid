use std::pin::pin;

use async_once_cell::OnceCell;
use async_std::io::{self, Stdout};
use async_std::sync::Mutex;
use orchid_base::msg::{recv_msg, send_msg};

static STDOUT: OnceCell<Mutex<Stdout>> = OnceCell::new();

pub async fn send_parent_msg(msg: &[u8]) -> io::Result<()> {
	let stdout_lk = STDOUT.get_or_init(async { Mutex::new(io::stdout()) }).await;
	let mut stdout_g = stdout_lk.lock().await;
	send_msg(pin!(&mut *stdout_g), msg).await
}
pub async fn recv_parent_msg() -> io::Result<Vec<u8>> { recv_msg(pin!(io::stdin())).await }
