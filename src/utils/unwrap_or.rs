#[macro_export]
macro_rules! unwrap_or {
  ($m:expr; $fail:expr) => {
    { if let Some(res) = ($m) {res} else {$fail} }
  }
}