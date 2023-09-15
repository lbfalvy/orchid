//! A stable implementation of the never and infallible results

/// An enum with no values
pub enum Never {}

/// An infallible result
pub type Always<T> = Result<T, Never>;

/// Wrap value in a result with an impossible failure mode
pub fn always<T>(t: T) -> Result<T, Never> { Ok(t) }

/// Take success value out of a result with an impossible failure mode
pub fn unwrap_always<T>(result: Result<T, Never>) -> T {
  result.unwrap_or_else(|_| unreachable!("Never has no values"))
}
