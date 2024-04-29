/// [Hierarchy] implementation key. The two implementors of this trait are
/// [Base] and [Subtype]. These types are assigned to [InHierarchy::Role] to
/// select the implementation of [Hierarchy].
pub trait HierarchyRole {}

/// A type-level boolean suitable to select conditional trait implementations.
/// Implementors are [True] and [False]
pub trait TLBool {}
/// [TLBool] value of `true`. The opposite is [False]
pub struct TLTrue;
impl TLBool for TLTrue {}
/// [TLBool] value of `false`. The opposite is [True]
pub struct TLFalse;
impl TLBool for TLFalse {}

/// Assign this type to [InHierarchy::Role] and implement [Descendant] to create
/// a subtype. These types can be upcast to their parent type, conditionally
/// downcast from it, and selected for [Descendant::Parent] by other types.
pub struct Subtype;
impl HierarchyRole for Subtype {}
/// Assign this type to [InHierarchy::Role] to create a base type. These types
/// are upcast only to themselves, but they can be selected in
/// [Descendant::Parent].
pub struct Base;
impl HierarchyRole for Base {}

/// A type that implements [Hierarchy]. Used to select implementations of traits
/// on the hierarchy
pub trait InHierarchy: Clone {
  /// Indicates that this hierarchy element is a leaf. Leaves can never have
  /// children
  type IsLeaf: TLBool;
  /// Indicates that this hierarchy element is a root. Roots can never have
  /// parents
  type IsRoot: TLBool;
}
/// A type that derives from a parent type.
pub trait Extends: InHierarchy<IsRoot = TLFalse> + Into<Self::Parent> {
  /// Specify the immediate parent of this type. This guides the
  type Parent: InHierarchy<IsLeaf = TLFalse>
    + TryInto<Self>
    + UnderRootImpl<<Self::Parent as InHierarchy>::IsRoot>;
}

pub trait UnderRootImpl<IsRoot: TLBool>: Sized {
  type __Root: UnderRoot<IsRoot = TLTrue, Root = Self::__Root>;
  fn __into_root(self) -> Self::__Root;
  fn __try_from_root(root: Self::__Root) -> Result<Self, Self::__Root>;
}

pub trait UnderRoot: InHierarchy {
  type Root: UnderRoot<IsRoot = TLTrue, Root = Self::Root>;
  fn into_root(self) -> Self::Root;
  fn try_from_root(root: Self::Root) -> Result<Self, Self::Root>;
}

impl<T: InHierarchy + UnderRootImpl<T::IsRoot>> UnderRoot for T {
  type Root = <Self as UnderRootImpl<<Self as InHierarchy>::IsRoot>>::__Root;
  fn into_root(self) -> Self::Root { self.__into_root() }
  fn try_from_root(root: Self::Root) -> Result<Self, Self::Root> { Self::__try_from_root(root) }
}

impl<T: InHierarchy<IsRoot = TLTrue>> UnderRootImpl<TLTrue> for T {
  type __Root = Self;
  fn __into_root(self) -> Self::__Root { self }
  fn __try_from_root(root: Self::__Root) -> Result<Self, Self::__Root> { Ok(root) }
}

impl<T: InHierarchy<IsRoot = TLFalse> + Extends> UnderRootImpl<TLFalse> for T {
  type __Root = <<Self as Extends>::Parent as UnderRootImpl<
    <<Self as Extends>::Parent as InHierarchy>::IsRoot,
  >>::__Root;
  fn __into_root(self) -> Self::__Root {
    <Self as Into<<Self as Extends>::Parent>>::into(self).into_root()
  }
  fn __try_from_root(root: Self::__Root) -> Result<Self, Self::__Root> {
    let parent = <Self as Extends>::Parent::try_from_root(root)?;
    parent.clone().try_into().map_err(|_| parent.into_root())
  }
}
