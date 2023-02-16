use super::Side;

/// The output of a two-part algorithm. The values are
/// 
/// - [Product2::Left] or [Product2::Right] if one of the arguments is the product
/// - [Product2::Either] if the arguments are identical
/// - [Product2::New] if the product is a different value from either
pub enum Product2<T> {
  Left,
  Right,
  Either,
  New(T)
}
impl<T> Product2<T> {
  /// Convert the product into a concrete value by providing the original arguments
  pub fn pick(self, left: T, right: T) -> T {
    match self {
      Self::Left | Self::Either => left,
      Self::Right => right,
      Self::New(t) => t
    }
  }

  /// Combine some subresults into a tuple representing a greater result
  pub fn join<U>(
    self, (lt, rt): (T, T),
    second: Product2<U>, (lu, ru): (U, U)
  ) -> Product2<(T, U)> {
    match (self, second) {
      (Self::Either, Product2::Either) => Product2::Either,
      (Self::Left | Self::Either, Product2::Left | Product2::Either) => Product2::Left,
      (Self::Right | Self::Either, Product2::Right | Product2::Either) => Product2::Right,
      (t, u) => Product2::New((t.pick(lt, rt), u.pick(lu, ru)))
    }
  }
  
  /// Translate results back into the type of the original problem.
  pub fn map<A, F: FnOnce(T) -> A>(self, f: F) -> Product2<A> {
    match self {
      Product2::Left => Product2::Left, Product2::Right => Product2::Right,
      Product2::Either => Product2::Either,
      Product2::New(t) => Product2::New(f(t))
    }
  }
}

/// Technically very different but sometimes neecessary to translate
impl<T> From<Side> for Product2<T> {
  fn from(value: Side) -> Self {match value {
    Side::Left => Self::Left,
    Side::Right => Self::Right
  }}
}