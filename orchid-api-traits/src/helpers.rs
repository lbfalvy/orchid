use std::future::Future;
use std::pin::Pin;

use async_std::io::{Read, ReadExt, Write, WriteExt};
use itertools::{Chunk, Itertools};

use crate::Encode;

pub async fn encode_enum<'a, W: Write + ?Sized, F: Future<Output = ()>>(
	mut write: Pin<&'a mut W>,
	id: u8,
	f: impl FnOnce(Pin<&'a mut W>) -> F,
) {
	id.encode(write.as_mut()).await;
	f(write).await
}

pub async fn write_exact<W: Write + ?Sized>(mut write: Pin<&mut W>, bytes: &'static [u8]) {
	write.write_all(bytes).await.expect("Failed to write exact bytes")
}

pub fn print_bytes(b: &[u8]) -> String {
	(b.iter().map(|b| format!("{b:02x}")))
		.chunks(4)
		.into_iter()
		.map(|mut c: Chunk<_>| c.join(" "))
		.join("  ")
}

pub async fn read_exact<R: Read + ?Sized>(mut read: Pin<&mut R>, bytes: &'static [u8]) {
	let mut data = vec![0u8; bytes.len()];
	read.read_exact(&mut data).await.expect("Failed to read bytes");
	if data != bytes {
		panic!("Wrong bytes!\nExpected: {}\nFound: {}", print_bytes(bytes), print_bytes(&data));
	}
}

pub async fn enc_vec(enc: &impl Encode) -> Vec<u8> {
	let mut vec = Vec::new();
	enc.encode(Pin::new(&mut vec)).await;
	vec
}
