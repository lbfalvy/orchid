use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Write};
use std::iter;
use std::ops::{Range, RangeInclusive};
use std::rc::Rc;
use std::sync::Arc;

use never::Never;
use ordered_float::NotNan;

use crate::encode_enum;

pub trait Decode {
	/// Decode an instance from the beginning of the buffer. Return the decoded
	/// data and the remaining buffer.
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self;
}
pub trait Encode {
	/// Append an instance of the struct to the buffer
	fn encode<W: Write + ?Sized>(&self, write: &mut W);
}
pub trait Coding: Encode + Decode + Clone {
	fn get_decoder<T>(map: impl Fn(Self) -> T + 'static) -> impl Fn(&mut dyn Read) -> T {
		move |r| map(Self::decode(r))
	}
}
impl<T: Encode + Decode + Clone> Coding for T {}

macro_rules! num_impl {
	($number:ty) => {
		impl Decode for $number {
			fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
				let mut bytes = [0u8; (<$number>::BITS / 8) as usize];
				read.read_exact(&mut bytes).unwrap();
				<$number>::from_be_bytes(bytes)
			}
		}
		impl Encode for $number {
			fn encode<W: Write + ?Sized>(&self, write: &mut W) {
				write.write_all(&self.to_be_bytes()).expect("Could not write number")
			}
		}
	};
}
num_impl!(u128);
num_impl!(u64);
num_impl!(u32);
num_impl!(u16);
num_impl!(u8);
num_impl!(i128);
num_impl!(i64);
num_impl!(i32);
num_impl!(i16);
num_impl!(i8);

macro_rules! nonzero_impl {
	($name:ty) => {
		impl Decode for $name {
			fn decode<R: Read + ?Sized>(read: &mut R) -> Self { Self::new(Decode::decode(read)).unwrap() }
		}
		impl Encode for $name {
			fn encode<W: Write + ?Sized>(&self, write: &mut W) { self.get().encode(write) }
		}
	};
}

nonzero_impl!(std::num::NonZeroU8);
nonzero_impl!(std::num::NonZeroU16);
nonzero_impl!(std::num::NonZeroU32);
nonzero_impl!(std::num::NonZeroU64);
nonzero_impl!(std::num::NonZeroU128);
nonzero_impl!(std::num::NonZeroI8);
nonzero_impl!(std::num::NonZeroI16);
nonzero_impl!(std::num::NonZeroI32);
nonzero_impl!(std::num::NonZeroI64);
nonzero_impl!(std::num::NonZeroI128);

impl<T: Encode + ?Sized> Encode for &T {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) { (**self).encode(write) }
}
macro_rules! float_impl {
	($t:ty, $size:expr) => {
		impl Decode for NotNan<$t> {
			fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
				let mut bytes = [0u8; $size];
				read.read_exact(&mut bytes).unwrap();
				NotNan::new(<$t>::from_be_bytes(bytes)).expect("Float was NaN")
			}
		}
		impl Encode for NotNan<$t> {
			fn encode<W: Write + ?Sized>(&self, write: &mut W) {
				write.write_all(&self.as_ref().to_be_bytes()).expect("Could not write number")
			}
		}
	};
}

float_impl!(f64, 8);
float_impl!(f32, 4);

impl Decode for String {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		let len = u64::decode(read).try_into().unwrap();
		let mut data = vec![0u8; len];
		read.read_exact(&mut data).unwrap();
		std::str::from_utf8(&data).expect("String invalid UTF-8").to_owned()
	}
}
impl Encode for String {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		u64::try_from(self.len()).unwrap().encode(write);
		write.write_all(self.as_bytes()).unwrap()
	}
}
impl Encode for str {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		u64::try_from(self.len()).unwrap().encode(write);
		write.write_all(self.as_bytes()).unwrap()
	}
}
impl<T: Decode> Decode for Vec<T> {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		let len = u64::decode(read).try_into().unwrap();
		iter::repeat_with(|| T::decode(read)).take(len).collect()
	}
}
impl<T: Encode> Encode for Vec<T> {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		u64::try_from(self.len()).unwrap().encode(write);
		self.iter().for_each(|t| t.encode(write));
	}
}
impl<T: Encode> Encode for [T] {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		u64::try_from(self.len()).unwrap().encode(write);
		self.iter().for_each(|t| t.encode(write));
	}
}
impl<T: Decode> Decode for Option<T> {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		match u8::decode(read) {
			0 => None,
			1 => Some(T::decode(read)),
			x => panic!("{x} is not a valid option value"),
		}
	}
}
impl<T: Encode> Encode for Option<T> {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		let t = if let Some(t) = self { t } else { return 0u8.encode(write) };
		1u8.encode(write);
		t.encode(write);
	}
}
impl<T: Decode, E: Decode> Decode for Result<T, E> {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		match u8::decode(read) {
			0 => Self::Ok(T::decode(read)),
			1 => Self::Err(E::decode(read)),
			x => panic!("Invalid Result tag {x}"),
		}
	}
}

impl<T: Encode, E: Encode> Encode for Result<T, E> {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		match self {
			Ok(t) => encode_enum(write, 0, |w| t.encode(w)),
			Err(e) => encode_enum(write, 1, |w| e.encode(w)),
		}
	}
}
impl<K: Decode + Eq + Hash, V: Decode> Decode for HashMap<K, V> {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		let len = u64::decode(read).try_into().unwrap();
		iter::repeat_with(|| <(K, V)>::decode(read)).take(len).collect()
	}
}
impl<K: Encode + Eq + Hash, V: Encode> Encode for HashMap<K, V> {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		u64::try_from(self.len()).unwrap().encode(write);
		self.iter().for_each(|pair| pair.encode(write));
	}
}
macro_rules! tuple {
  (($($t:ident)*) ($($T:ident)*)) => {
    impl<$($T: Decode),*> Decode for ($($T,)*) {
      fn decode<R: Read + ?Sized>(read: &mut R) -> Self { ($($T::decode(read),)*) }
    }
    impl<$($T: Encode),*> Encode for ($($T,)*) {
      fn encode<W: Write + ?Sized>(&self, write: &mut W) {
        let ($($t,)*) = self;
        $( $t.encode(write); )*
      }
    }
  };
}

tuple!((t)(T));
tuple!((t u) (T U));
tuple!((t u v) (T U V));
tuple!((t u v x) (T U V X)); // 4
tuple!((t u v x y) (T U V X Y));
tuple!((t u v x y z) (T U V X Y Z));
tuple!((t u v x y z a) (T U V X Y Z A));
tuple!((t u v x y z a b) (T U V X Y Z A B)); // 8
tuple!((t u v x y z a b c) (T U V X Y Z A B C));
tuple!((t u v x y z a b c d) (T U V X Y Z A B C D));
tuple!((t u v x y z a b c d e) (T U V X Y Z A B C D E));
tuple!((t u v x y z a b c d e f) (T U V X Y Z A B C D E F)); // 12
tuple!((t u v x y z a b c d e f g) (T U V X Y Z A B C D E F G));
tuple!((t u v x y z a b c d e f g h) (T U V X Y Z A B C D E F G H));
tuple!((t u v x y z a b c d e f g h i) (T U V X Y Z A B C D E F G H I));
tuple!((t u v x y z a b c d e f g h i j) (T U V X Y Z A B C D E F G H I J)); // 16

impl Decode for () {
	fn decode<R: Read + ?Sized>(_: &mut R) -> Self {}
}
impl Encode for () {
	fn encode<W: Write + ?Sized>(&self, _: &mut W) {}
}
impl Decode for Never {
	fn decode<R: Read + ?Sized>(_: &mut R) -> Self {
		unreachable!("A value of Never cannot exist so it can't have been serialized");
	}
}
impl Encode for Never {
	fn encode<W: Write + ?Sized>(&self, _: &mut W) { match *self {} }
}
impl Decode for bool {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		let mut buf = [0];
		read.read_exact(&mut buf).unwrap();
		buf[0] != 0
	}
}
impl Encode for bool {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) {
		write.write_all(&[if *self { 0xff } else { 0 }]).unwrap()
	}
}
impl<T: Decode, const N: usize> Decode for [T; N] {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
		// TODO: figure out how to do this in safe rust on the stack
		((0..N).map(|_| T::decode(read)).collect::<Vec<_>>().try_into())
			.unwrap_or_else(|_| unreachable!("The length of this iterator is statically known"))
	}
}
impl<T: Encode, const N: usize> Encode for [T; N] {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) { self.iter().for_each(|t| t.encode(write)) }
}

macro_rules! two_end_range {
  ($this:ident, $name:tt, $op:tt, $start:expr, $end:expr) => {
    impl<T: Decode> Decode for $name<T> {
      fn decode<R: Read + ?Sized>(read: &mut R) -> Self { T::decode(read) $op T::decode(read) }
    }
    impl<T: Encode> Encode for $name<T> {
      fn encode<W: Write + ?Sized>(&self, write: &mut W) {
        let $this = self;
        ($start).encode(write);
        ($end).encode(write);
      }
    }
  }
}

two_end_range!(x, Range, .., x.start, x.end);
two_end_range!(x, RangeInclusive, ..=, x.start(), x.end());

macro_rules! smart_ptr {
	($name:tt) => {
		impl<T: Decode> Decode for $name<T> {
			fn decode<R: Read + ?Sized>(read: &mut R) -> Self { $name::new(T::decode(read)) }
		}
		impl<T: Encode> Encode for $name<T> {
			fn encode<W: Write + ?Sized>(&self, write: &mut W) { (**self).encode(write) }
		}
	};
}

smart_ptr!(Arc);
smart_ptr!(Rc);
smart_ptr!(Box);

impl<T: ?Sized + ToOwned> Decode for Cow<'_, T>
where T::Owned: Decode
{
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self { Cow::Owned(T::Owned::decode(read)) }
}
impl<T: ?Sized + Encode + ToOwned> Encode for Cow<'_, T> {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) { (**self).encode(write) }
}

impl Decode for char {
	fn decode<R: Read + ?Sized>(read: &mut R) -> Self { char::from_u32(u32::decode(read)).unwrap() }
}
impl Encode for char {
	fn encode<W: Write + ?Sized>(&self, write: &mut W) { (*self as u32).encode(write) }
}
