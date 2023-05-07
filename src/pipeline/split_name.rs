use crate::interner::Token;

pub fn split_name<'a>(
  path: &'a [Token<String>],
  is_valid: &impl Fn(&[Token<String>]) -> bool
) -> Option<(&'a [Token<String>], &'a [Token<String>])> {
  for split in (0..=path.len()).rev() {
    let (filename, subpath) = path.split_at(split);
    if is_valid(filename) {
      return Some((filename, subpath))
    }
  }
  None
}