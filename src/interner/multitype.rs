use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::cell::{RefCell, RefMut};
use std::hash::Hash;
use std::rc::Rc;

use hashbrown::HashMap;

use super::monotype::TypedInterner;
use super::token::Tok;
// use super::InternedDisplay;

/// A collection of interners based on their type. Allows to intern any object
/// that implements [ToOwned]. Objects of the same type are stored together in a
/// [TypedInterner].
pub struct Interner {
  interners: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
}
impl Interner {
  /// Create a new interner
  pub fn new() -> Self {
    Self { interners: RefCell::new(HashMap::new()) }
  }

  /// Intern something
  pub fn i<Q: ?Sized + Eq + Hash + ToOwned>(&self, q: &Q) -> Tok<Q::Owned>
  where
    Q::Owned: 'static + Eq + Hash + Clone + Borrow<Q>,
  {
    let mut interners = self.interners.borrow_mut();
    let interner = get_interner(&mut interners);
    interner.i(q)
  }

  /// Fully resolve a list of interned things.
  pub fn extern_all<T: 'static + Eq + Hash + Clone>(s: &[Tok<T>]) -> Vec<T> {
    s.iter().map(|t| (**t).clone()).collect()
  }

  // /// A variant of `unwrap` using [InternedDisplay] to circumvent `unwrap`'s
  // /// dependencyon [Debug]. For clarity, [expect] should be preferred.
  // pub fn unwrap<T, E: InternedDisplay>(&self, result: Result<T, E>) -> T {
  //   result.unwrap_or_else(|e| {
  //     println!("Unwrapped Error: {}", e.bundle(self));
  //     panic!("Unwrapped an error");
  //   })
  // }

  // /// A variant of `expect` using  [InternedDisplay] to circumvent `expect`'s
  // /// depeendency on [Debug].
  // pub fn expect<T, E: InternedDisplay>(
  //   &self,
  //   result: Result<T, E>,
  //   msg: &str,
  // ) -> T {
  //   result.unwrap_or_else(|e| {
  //     println!("Expectation failed: {msg}");
  //     println!("Error: {}", e.bundle(self));
  //     panic!("Expected an error");
  //   })
  // }
}

impl Default for Interner {
  fn default() -> Self {
    Self::new()
  }
}

/// Get or create an interner for a given type.
fn get_interner<T: 'static + Eq + Hash + Clone>(
  interners: &mut RefMut<HashMap<TypeId, Rc<dyn Any>>>,
) -> Rc<TypedInterner<T>> {
  let boxed = interners
    .raw_entry_mut()
    .from_key(&TypeId::of::<T>())
    .or_insert_with(|| (TypeId::of::<T>(), TypedInterner::<T>::new()))
    .1
    .clone();
  boxed.downcast().expect("the typeid is supposed to protect from this")
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn test_string() {
    let interner = Interner::new();
    let key1 = interner.i("foo");
    let key2 = interner.i(&"foo".to_string());
    assert_eq!(key1, key2)
  }

  #[test]
  pub fn test_slice() {
    let interner = Interner::new();
    let key1 = interner.i(&vec![1, 2, 3]);
    let key2 = interner.i(&[1, 2, 3][..]);
    assert_eq!(key1, key2);
  }

  // #[test]
  #[allow(unused)]
  pub fn test_str_slice() {
    let interner = Interner::new();
    let key1 =
      interner.i(&vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let key2 = interner.i(&["a", "b", "c"][..]);
    // assert_eq!(key1, key2);
  }
}
