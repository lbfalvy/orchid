/// A macro version of [Option::unwrap_or_else] which supports flow
/// control statements such as `return` and `break` in the "else" branch.
///
/// ```ignore
/// crate::unwrap_or!(Some(1); return)
/// ```
///
/// It also supports unwrapping concrete variants of other enums
///
/// ```ignore
/// use crate::representations::Literal;
///
/// crate::unwrap_or!(Literal::Usize(2) => Literal::Number; return)
/// ```
macro_rules! unwrap_or {
  ($m:expr; $fail:expr) => {{
    if let Some(res) = ($m) { res } else { $fail }
  }};
  ($m:expr => $pattern:path; $fail:expr) => {{
    if let $pattern(res) = ($m) { res } else { $fail }
  }};
}

pub(crate) use unwrap_or;
