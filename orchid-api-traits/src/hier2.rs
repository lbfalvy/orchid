pub trait TBool {}
pub struct TTrue;
impl TBool for TTrue {}
pub struct TFalse;
impl TBool for TFalse {}

/// Implementation picker for a tree node
///
/// Note: This technically allows for the degenerate case
/// ```
/// struct MyType;
/// impl TreeRolePicker for MyType {
///   type IsLeaf = TTrue;
///   type IsRoot = TTrue;
/// }
/// ```
/// This isn't very useful because it describes a one element sealed hierarchy.
pub trait TreeRolePicker {
  type IsLeaf: TBool;
  type IsRoot: TBool;
}

pub trait Extends: TreeRolePicker<IsRoot = TFalse> {
  type Parent: TreeRolePicker<IsLeaf = TFalse>;
}

pub trait Inherits<T> {}

// impl<T> Inherits<T, 0> for T {}
impl<T: Extends, This> Inherits<T::Parent> for This where This: Inherits<T> {}
