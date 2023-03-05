use std::iter;

pub fn replace_first<'a, T, F>(slice: &'a [T], mut f: F) -> Option<impl Iterator<Item = T> + 'a>
where T: Clone, F: FnMut(&T) -> Option<T> {
  for i in 0..slice.len() {
    if let Some(new) = f(&slice[i]) {
      let subbed_iter = slice[0..i].iter().cloned()
        .chain(iter::once(new))
        .chain(slice[i+1..].iter().cloned());
      return Some(subbed_iter)
    }
  }
  None
}