use std::mem;

pub fn translate<T, F: FnOnce(T) -> T>(data: &mut T, f: F) {
  unsafe {
    let mut acc = mem::MaybeUninit::<T>::uninit().assume_init();
    mem::swap(&mut acc, data);
    let mut new = f(acc);
    mem::swap(&mut new, data);
    mem::forget(new);
  }
}

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