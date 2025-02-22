use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::num::NonZero;
use std::ops::{Range, RangeInclusive};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use async_std::io::{Read, ReadExt, Write, WriteExt};
use async_stream::stream;
use futures::StreamExt;
use never::Never;
use ordered_float::NotNan;

use crate::encode_enum;

pub trait Decode: 'static {
	/// Decode an instance from the beginning of the buffer. Return the decoded
	/// data and the remaining buffer.
	fn decode<R: Read + ?Sized>(read: Pin<&mut R>) -> impl Future<Output = Self> + '_;
}
pub trait Encode {
	/// Append an instance of the struct to the buffer
	fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) -> impl Future<Output = ()>;
}
pub trait Coding: Encode + Decode + Clone {
	fn get_decoder<T: 'static, F: Future<Output = T> + 'static>(
		map: impl Fn(Self) -> F + Clone + 'static,
	) -> impl AsyncFn(Pin<&mut dyn Read>) -> T {
		async move |r| map(Self::decode(r).await).await
	}
}
impl<T: Encode + Decode + Clone> Coding for T {}

macro_rules! num_impl {
	($number:ty) => {
		impl Decode for $number {
			async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
				let mut bytes = [0u8; (<$number>::BITS / 8) as usize];
				read.read_exact(&mut bytes).await.unwrap();
				<$number>::from_be_bytes(bytes)
			}
		}
		impl Encode for $number {
			async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
				write.write_all(&self.to_be_bytes()).await.expect("Could not write number")
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
		impl Decode for NonZero<$name> {
			async fn decode<R: Read + ?Sized>(read: Pin<&mut R>) -> Self {
				Self::new(<$name as Decode>::decode(read).await).unwrap()
			}
		}
		impl Encode for NonZero<$name> {
			async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) {
				self.get().encode(write).await
			}
		}
	};
}

nonzero_impl!(u8);
nonzero_impl!(u16);
nonzero_impl!(u32);
nonzero_impl!(u64);
nonzero_impl!(u128);
nonzero_impl!(i8);
nonzero_impl!(i16);
nonzero_impl!(i32);
nonzero_impl!(i64);
nonzero_impl!(i128);

impl<T: Encode + ?Sized> Encode for &T {
	async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) { (**self).encode(write).await }
}
macro_rules! float_impl {
	($t:ty, $size:expr) => {
		impl Decode for NotNan<$t> {
			async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
				let mut bytes = [0u8; $size];
				read.read_exact(&mut bytes).await.unwrap();
				NotNan::new(<$t>::from_be_bytes(bytes)).expect("Float was NaN")
			}
		}
		impl Encode for NotNan<$t> {
			async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
				write.write_all(&self.as_ref().to_be_bytes()).await.expect("Could not write number")
			}
		}
	};
}

float_impl!(f64, 8);
float_impl!(f32, 4);

impl Decode for String {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		let len = u64::decode(read.as_mut()).await.try_into().unwrap();
		let mut data = vec![0u8; len];
		read.read_exact(&mut data).await.unwrap();
		std::str::from_utf8(&data).expect("String invalid UTF-8").to_owned()
	}
}
impl Encode for String {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		u64::try_from(self.len()).unwrap().encode(write.as_mut()).await;
		write.write_all(self.as_bytes()).await.unwrap()
	}
}
impl Encode for str {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		u64::try_from(self.len()).unwrap().encode(write.as_mut()).await;
		write.write_all(self.as_bytes()).await.unwrap()
	}
}
impl<T: Decode> Decode for Vec<T> {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		let len = u64::decode(read.as_mut()).await.try_into().unwrap();
		stream! { loop { yield T::decode(read.as_mut()).await } }.take(len).collect().await
	}
}
impl<T: Encode> Encode for Vec<T> {
	async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) {
		self.as_slice().encode(write).await
	}
}
impl<T: Encode> Encode for [T] {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		u64::try_from(self.len()).unwrap().encode(write.as_mut()).await;
		for t in self.iter() {
			t.encode(write.as_mut()).await
		}
	}
}
impl<T: Decode> Decode for Option<T> {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		match u8::decode(read.as_mut()).await {
			0 => None,
			1 => Some(T::decode(read).await),
			x => panic!("{x} is not a valid option value"),
		}
	}
}
impl<T: Encode> Encode for Option<T> {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		let t = if let Some(t) = self { t } else { return 0u8.encode(write.as_mut()).await };
		1u8.encode(write.as_mut()).await;
		t.encode(write).await;
	}
}
impl<T: Decode, E: Decode> Decode for Result<T, E> {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		match u8::decode(read.as_mut()).await {
			0 => Self::Ok(T::decode(read).await),
			1 => Self::Err(E::decode(read).await),
			x => panic!("Invalid Result tag {x}"),
		}
	}
}

impl<T: Encode, E: Encode> Encode for Result<T, E> {
	async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) {
		match self {
			Ok(t) => encode_enum(write, 0, |w| t.encode(w)).await,
			Err(e) => encode_enum(write, 1, |w| e.encode(w)).await,
		}
	}
}
impl<K: Decode + Eq + Hash, V: Decode> Decode for HashMap<K, V> {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		let len = u64::decode(read.as_mut()).await.try_into().unwrap();
		stream! { loop { yield <(K, V)>::decode(read.as_mut()).await } }.take(len).collect().await
	}
}
impl<K: Encode + Eq + Hash, V: Encode> Encode for HashMap<K, V> {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		u64::try_from(self.len()).unwrap().encode(write.as_mut()).await;
		for pair in self.iter() {
			pair.encode(write.as_mut()).await
		}
	}
}
macro_rules! tuple {
  (($($t:ident)*) ($($T:ident)*)) => {
    impl<$($T: Decode),*> Decode for ($($T,)*) {
			async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
				($($T::decode(read.as_mut()).await,)*)
			}
    }
    impl<$($T: Encode),*> Encode for ($($T,)*) {
			async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
        let ($($t,)*) = self;
        $( $t.encode(write.as_mut()).await; )*
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
	async fn decode<R: Read + ?Sized>(_: Pin<&mut R>) -> Self {}
}
impl Encode for () {
	async fn encode<W: Write + ?Sized>(&self, _: Pin<&mut W>) {}
}
impl Decode for Never {
	async fn decode<R: Read + ?Sized>(_: Pin<&mut R>) -> Self {
		unreachable!("A value of Never cannot exist so it can't have been serialized");
	}
}
impl Encode for Never {
	async fn encode<W: Write + ?Sized>(&self, _: Pin<&mut W>) { match *self {} }
}
impl Decode for bool {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		let mut buf = [0];
		read.read_exact(&mut buf).await.unwrap();
		buf[0] != 0
	}
}
impl Encode for bool {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		write.write_all(&[if *self { 0xffu8 } else { 0u8 }]).await.unwrap()
	}
}
impl<T: Decode, const N: usize> Decode for [T; N] {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		// TODO: figure out how to do this in safe rust on the stack
		let v =
			stream! { loop { yield T::decode(read.as_mut()).await } }.take(N).collect::<Vec<_>>().await;
		v.try_into().unwrap_or_else(|_| unreachable!("The length of this stream is statically known"))
	}
}
impl<T: Encode, const N: usize> Encode for [T; N] {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		for t in self.iter() {
			t.encode(write.as_mut()).await
		}
	}
}

macro_rules! two_end_range {
  ($this:ident, $name:tt, $op:tt, $start:expr, $end:expr) => {
    impl<T: Decode> Decode for $name<T> {
			async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
				T::decode(read.as_mut()).await $op T::decode(read).await
			}
    }
    impl<T: Encode> Encode for $name<T> {
			async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
        let $this = self;
        ($start).encode(write.as_mut()).await;
        ($end).encode(write).await;
      }
    }
  }
}

two_end_range!(x, Range, .., x.start, x.end);
two_end_range!(x, RangeInclusive, ..=, x.start(), x.end());

macro_rules! smart_ptr {
	($name:tt) => {
		impl<T: Decode> Decode for $name<T> {
			async fn decode<R: Read + ?Sized>(read: Pin<&mut R>) -> Self {
				$name::new(T::decode(read).await)
			}
		}
		impl<T: Encode> Encode for $name<T> {
			async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) { (**self).encode(write).await }
		}
	};
}

smart_ptr!(Arc);
smart_ptr!(Rc);
smart_ptr!(Box);

impl Decode for char {
	async fn decode<R: Read + ?Sized>(read: Pin<&mut R>) -> Self {
		char::from_u32(u32::decode(read).await).unwrap()
	}
}
impl Encode for char {
	async fn encode<W: Write + ?Sized>(&self, write: Pin<&mut W>) {
		(*self as u32).encode(write).await
	}
}
