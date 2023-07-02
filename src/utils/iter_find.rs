/// Check if the finite sequence produced by a clonable iterator (`haystack`)
/// contains the finite sequence produced by another clonable iterator
/// (`needle`)
pub fn iter_find<T: Eq>(
  mut haystack: impl Iterator<Item = T> + Clone,
  needle: impl Iterator<Item = T> + Clone,
) -> Option<usize> {
  let mut start = 0;
  loop {
    match iter_starts_with(haystack.clone(), needle.clone()) {
      ISWResult::StartsWith => return Some(start),
      ISWResult::Shorter => return None,
      ISWResult::Difference => (),
    }
    haystack.next();
    start += 1;
  }
}

/// Value returned by iter_starts_with
enum ISWResult {
  /// The first iterator starts with the second
  StartsWith,
  /// The values of the two iterators differ
  Difference,
  /// The first iterator ends before the second
  Shorter,
}

/// Checks that an iterator starts with another
fn iter_starts_with<T: Eq>(
  mut a: impl Iterator<Item = T>,
  b: impl Iterator<Item = T>,
) -> ISWResult {
  // if a starts with b then for every element in b
  for item in b {
    // a has to contain the same element
    if let Some(comp) = a.next() {
      if item != comp {
        return ISWResult::Difference;
      }
    } else {
      return ISWResult::Shorter;
    }
  }
  ISWResult::StartsWith
}
