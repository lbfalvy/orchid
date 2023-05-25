/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided vector with the
/// element appended
pub fn pushed<T: Clone>(vec: &Vec<T>, t: T) -> Vec<T> {
  let mut next = Vec::with_capacity(vec.len() + 1);
  next.extend_from_slice(&vec[..]);
  next.push(t);
  next
}
