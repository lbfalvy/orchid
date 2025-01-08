use std::io::{Read, Write};

use itertools::{Chunk, Itertools};

use crate::Encode;

pub fn encode_enum<W: Write + ?Sized>(write: &mut W, id: u8, f: impl FnOnce(&mut W)) {
	id.encode(write);
	f(write)
}

pub fn write_exact<W: Write + ?Sized>(write: &mut W, bytes: &'static [u8]) {
	write.write_all(bytes).expect("Failed to write exact bytes")
}

pub fn print_bytes(b: &[u8]) -> String {
	(b.iter().map(|b| format!("{b:02x}")))
		.chunks(4)
		.into_iter()
		.map(|mut c: Chunk<_>| c.join(" "))
		.join("  ")
}

pub fn read_exact<R: Read + ?Sized>(read: &mut R, bytes: &'static [u8]) {
	let mut data = vec![0u8; bytes.len()];
	read.read_exact(&mut data).expect("Failed to read bytes");
	if data != bytes {
		panic!("Wrong bytes!\nExpected: {}\nFound: {}", print_bytes(bytes), print_bytes(&data));
	}
}

pub fn enc_vec(enc: &impl Encode) -> Vec<u8> {
	let mut vec = Vec::new();
	enc.encode(&mut vec);
	vec
}
