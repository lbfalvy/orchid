use std::borrow::Borrow;
use std::cell::{RefCell, RefMut};
use std::any::{TypeId, Any};
use std::hash::Hash;
use std::rc::Rc;

use hashbrown::HashMap;

use super::monotype::TypedInterner;
use super::token::Token;

pub struct Interner {
  interners: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
}
impl Interner {
  pub fn new() -> Self {
    Self { interners: RefCell::new(HashMap::new()) }
  }

  pub fn i<Q: ?Sized>(&self, q: &Q) -> Token<Q::Owned>
  where Q: Eq + Hash + ToOwned,
    Q::Owned: 'static + Eq + Hash + Clone,
    Q::Owned: Borrow<Q>
  {
    let mut interners = self.interners.borrow_mut();
    let interner = get_interner(&mut interners);
    interner.i(q)
  }

  pub fn r<T: 'static + Eq + Hash + Clone>(&self, t: Token<T>) -> &T {
    let mut interners = self.interners.borrow_mut();
    let interner = get_interner(&mut interners);
    // TODO: figure this out
    unsafe{ (interner.r(t) as *const T).as_ref().unwrap() }
  }

  /// Fully resolve
  /// TODO: make this generic over containers
  pub fn extern_vec<T: 'static + Eq + Hash + Clone>(&self,
    t: Token<Vec<Token<T>>>
  ) -> Vec<T> {
    let mut interners = self.interners.borrow_mut();
    let v_int = get_interner(&mut interners);
    let t_int = get_interner(&mut interners);
    let v = v_int.r(t);
    v.iter()
    .map(|t| t_int.r(*t))
    .cloned()
    .collect()
  }

  pub fn extern_all<T: 'static + Eq + Hash + Clone>(&self,
    s: &[Token<T>]
  ) -> Vec<T> {
    s.iter()
    .map(|t| self.r(*t))
    .cloned()
    .collect()
  }
}

/// Get or create an interner for a given type.
fn get_interner<T: 'static + Eq + Hash + Clone>(
  interners: &mut RefMut<HashMap<TypeId, Rc<dyn Any>>>
) -> Rc<TypedInterner<T>> {
  let boxed = interners.raw_entry_mut().from_key(&TypeId::of::<T>())
    .or_insert_with(|| (
      TypeId::of::<T>(),
      Rc::new(TypedInterner::<T>::new())
    )).1.clone();
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
    let key1 = interner.i(&vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let key2 = interner.i(&["a", "b", "c"][..]);
    // assert_eq!(key1, key2);
  }
}