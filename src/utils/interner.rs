use std::sync::{Mutex, Arc};
use std::num::NonZeroU32;
use std::hash::Hash;

use lasso::{Rodeo, Spur, Key};
use base64::{engine::general_purpose::STANDARD_NO_PAD as BASE64, Engine};

/// A token representing an interned string or sequence in an interner
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Token<const RANK: u8>(pub Spur);

/// An interner that can intern strings, and sequences of things it
/// interned as long as they have the same rank
pub trait Interner: Sync {
  fn str2tok(&self, str: &str) -> Token<String>;
  fn tok2str(&self, token: Token<String>) -> String;
  fn slc2tok<const RANK: u8>(&self, slice: &[Token<RANK>]) -> Token<{RANK + 1}>;
  fn tok2slc<const RANK: u8>(&self, token: Token<RANK>) -> Vec<Token<{RANK - 1}>>;
  fn tok2strv(&self, token: Token<Vec<Token<String>>>) -> Vec<String> {
    self.tok2slc(token).into_iter().map(|t| self.tok2str(t)).collect()
  }
  fn tokv2strv(&self, slice: &[Token<String>]) -> Vec<String> {
    slice.iter().map(|t| self.tok2str(*t)).collect()
  }
  /// Get the first token of a sequence
  fn head<const RANK: u8>(&self, token: Token<RANK>) -> Token<{RANK - 1}>;
  /// Returns the length of a sequence
  fn len<const RANK: u8>(&self, token: Token<RANK>) -> usize
  where Token<{RANK - 1}>: Clone;
  /// Returns the length of the longest identical prefix of the two sequences
  fn coprefix<const RANK: u8>(&self, a: Token<RANK>, b: Token<RANK>) -> usize
  where Token<{RANK - 1}>: Clone;
}

fn serialize_seq<const RANK: u8>(seq: &[Token<RANK>]) -> String {
  let data: Vec<u8> = seq.iter()
    .map(|t| u32::from(t.0.into_inner()).to_le_bytes().into_iter())
    .flatten()
    .collect();
  BASE64.encode(data)
}

fn deserialize_seq<const RANK: u8>(string: &str) -> Vec<Token<RANK>> {
  let data = BASE64.decode(string)
    .expect("String is not valid base64");
  assert!(data.len() % 4 == 0, "Tokens should serialize to 3 bytes each");
  data.array_chunks::<4>().map(|a| {
    let bytes = [a[0], a[1], a[2], a[3]];
    let nz32 = NonZeroU32::new(u32::from_le_bytes(bytes))
      .expect("Token representation should never be zero");
    Token(Spur::try_from_usize(u32::from(nz32) as usize).unwrap())
  }).collect()
}

/// An interner that delegates the actual work to Lasso
#[derive(Clone)]
pub struct LassoInterner {
  strings: Arc<Mutex<Rodeo>>,
  slices: Arc<Mutex<Rodeo>>
}

impl LassoInterner {
  /// Create an empty interner. Called to create the singleton.
  fn new() -> Self {
    Self{
      slices: Arc::new(Mutex::new(Rodeo::new())),
      strings: Arc::new(Mutex::new(Rodeo::new()))
    }
  }
}

impl Interner for LassoInterner {
  fn str2tok(&self, str: &str) -> Token<String> {
    let mut strings = self.strings.lock().unwrap();
    let key = strings.get_or_intern(str);
    Token(key)
  }

  fn tok2str<'a>(&'a self, token: Token<String>) -> String {
    let key = token.0;
    let strings = self.strings.lock().unwrap();
    strings.resolve(&key).to_string()
  }

  fn slc2tok<const RANK: u8>(&self, slice: &[Token<RANK>]) -> Token<{RANK + 1}> {
    let data = serialize_seq(slice);
    let mut slices = self.slices.lock().unwrap();
    let key = slices.get_or_intern(data);
    Token(key)
  }

  fn tok2slc<'a, const RANK: u8>(&'a self, token: Token<RANK>) -> Vec<Token<{RANK - 1}>> {
    let key = token.0;
    let slices = self.slices.lock().unwrap();
    let string = slices.resolve(&key);
    deserialize_seq(string)
  }

  fn head<const RANK: u8>(&self, token: Token<RANK>) -> Token<{RANK - 1}> {
    let key = token.0;
    let slices = self.slices.lock().unwrap();
    let string = slices.resolve(&key);
    deserialize_seq(&string[0..5])[0]
  }

  fn len<const RANK: u8>(&self, token: Token<RANK>) -> usize where Token<{RANK - 1}>: Clone {
    let key = token.0;
    let slices = self.slices.lock().unwrap();
    let string = slices.resolve(&key);
    assert!(string.len() % 4 == 0, "Tokens should serialize to 3 characters");
    string.len() / 4
  }

  fn coprefix<const RANK: u8>(&self, a: Token<RANK>, b: Token<RANK>) -> usize where Token<{RANK - 1}>: Clone {
    let keya = a.0;
    let keyb = b.0;
    let slices = self.slices.lock().unwrap();
    let sa = slices.resolve(&keya);
    let sb = slices.resolve(&keyb);
    sa.bytes()
      .zip(sb.bytes())
      .take_while(|(a, b)| a == b)
      .count() / 4
  }
}

/// Create an interner that inherits the singleton's data, and
/// block all future interaction with the singleton.
/// 
/// DO NOT call within [dynamic] or elsewhere pre-main
pub fn mk_interner() -> impl Interner {
  LassoInterner::new()
}

pub trait StringLike: Clone + Eq + Hash {
  fn into_str(self, i: &Interner) -> String;
  fn into_tok(self, i: &Interner) -> Token<String>;
}

impl StringLike for String {
  fn into_str(self, _i: &Interner) -> String {self}
  fn into_tok(self, i: &Interner) -> Token<String> {i.str2tok(&self)}
}

impl StringLike for Token<String> {
  fn into_str(self, i: &Interner) -> String {i.tok2str(self)}
  fn into_tok(self, _i: &Interner) -> Token<String> {self}
}

pub trait StringVLike: Clone + Eq + Hash {
  fn into_strv(self, i: &Interner) -> Vec<String>;
  fn into_tok(self, i: &Interner) -> Token<Vec<Token<String>>>;
  fn into_tokv(self, i: &Interner) -> Vec<Token<String>>;
}

impl StringVLike for Vec<String> {
  fn into_strv(self, _i: &Interner) -> Vec<String> {self}
  fn into_tok(self, i: &Interner) -> Token<Vec<Token<String>>> {
    let tokv = self.into_iter()
      .map(|s| i.str2tok(&s))
      .collect::<Vec<_>>();
    i.slc2tok(&tokv)
  }
  fn into_tokv(self, i: &Interner) -> Vec<Token<String>> {
    self.into_iter()
      .map(|s| i.str2tok(&s))
      .collect()
  }
}

impl StringVLike for Vec<Token<String>> {
  fn into_strv(self, i: &Interner) -> Vec<String> {i.tokv2strv(&self)}
  fn into_tok(self, i: &Interner) -> Token<Vec<Token<String>>> {i.slc2tok(&self)}
  fn into_tokv(self, _i: &Interner) -> Vec<Token<String>> {self}
}

impl StringVLike for Token<Vec<Token<String>>> {
  fn into_strv(self, i: &Interner) -> Vec<String> {i.tok2strv(self)}
  fn into_tok(self, _i: &Interner) -> Token<Vec<Token<String>>> {self}
  fn into_tokv(self, i: &Interner) -> Vec<Token<String>> {i.tok2slc(self)}
}