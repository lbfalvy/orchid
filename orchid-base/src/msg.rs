use std::io;
use std::pin::Pin;

use async_std::io::{Read, ReadExt, Write, WriteExt};
use orchid_api_traits::{Decode, Encode};

pub async fn send_msg(mut write: Pin<&mut impl Write>, msg: &[u8]) -> io::Result<()> {
	let mut len_buf = vec![];
	u32::try_from(msg.len()).unwrap().encode(&mut len_buf);
	write.write_all(&len_buf).await?;
	write.write_all(msg).await?;
	write.flush().await
}

pub async fn recv_msg(mut read: Pin<&mut impl Read>) -> io::Result<Vec<u8>> {
	let mut len_buf = [0u8; (u32::BITS / 8) as usize];
	read.read_exact(&mut len_buf).await?;
	let len = u32::decode(&mut &len_buf[..]);
	let mut msg = vec![0u8; len as usize];
	read.read_exact(&mut msg).await?;
	Ok(msg)
}
