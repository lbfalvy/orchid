use std::mem;

// TODO: extract to crate

#[allow(unused)]
/// Map over a `&mut` with a mapper function that takes ownership of
/// the value
pub fn translate<T, F: FnOnce(T) -> T>(data: &mut T, f: F) {
  unsafe {
    let mut acc = mem::MaybeUninit::<T>::uninit().assume_init();
    mem::swap(&mut acc, data);
    let mut new = f(acc);
    mem::swap(&mut new, data);
    mem::forget(new);
  }
}

/// Map over a `&mut` with a mapper function that takes ownership of
/// the value and also produces some unrelated data.
pub fn process<T, U, F: FnOnce(T) -> (T, U)>(data: &mut T, f: F) -> U {
  unsafe {
    let mut acc = mem::MaybeUninit::<T>::uninit().assume_init();
    mem::swap(&mut acc, data);
    let (mut new, ret) = f(acc);
    mem::swap(&mut new, data);
    mem::forget(new);
    ret
  }
}