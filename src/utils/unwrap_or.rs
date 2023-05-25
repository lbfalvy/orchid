/// A macro version of [Option::unwrap_or_else] which supports flow
/// control statements such as `return` and `break` in the "else" branch.
#[macro_export]
macro_rules! unwrap_or {
  ($m:expr; $fail:expr) => {{
    if let Some(res) = ($m) { res } else { $fail }
  }};
}
