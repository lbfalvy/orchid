use crate::interner::Tok;

#[allow(clippy::type_complexity)]
// FIXME couldn't find a good factoring
pub fn split_name<'a>(
  path: &'a [Tok<String>],
  is_valid: &impl Fn(&[Tok<String>]) -> bool,
) -> Option<(&'a [Tok<String>], &'a [Tok<String>])> {
  for split in (0..=path.len()).rev() {
    let (filename, subpath) = path.split_at(split);
    if is_valid(filename) {
      return Some((filename, subpath));
    }
  }
  None
}
