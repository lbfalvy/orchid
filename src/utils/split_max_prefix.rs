/// Split off the longest prefix accepted by the validator
pub fn split_max_prefix<'a, T>(
  path: &'a [T],
  is_valid: &impl Fn(&[T]) -> bool,
) -> Option<(&'a [T], &'a [T])> {
  (0..=path.len())
    .rev()
    .map(|i| path.split_at(i))
    .find(|(file, _)| is_valid(file))
}
