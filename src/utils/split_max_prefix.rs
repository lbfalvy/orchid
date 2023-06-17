/// Split off the longest prefix accepted by the validator
#[allow(clippy::type_complexity)] // FIXME couldn't find a good factoring
pub fn split_max_prefix<'a, T>(
  path: &'a [T],
  is_valid: &impl Fn(&[T]) -> bool,
) -> Option<(&'a [T], &'a [T])> {
  for split in (0..=path.len()).rev() {
    let (filename, subpath) = path.split_at(split);
    if is_valid(filename) {
      return Some((filename, subpath));
    }
  }
  None
}
