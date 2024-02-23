//! Generic module tree structure
//!
//! Used by various stages of the pipeline with different parameters
use std::fmt;

use hashbrown::HashMap;
use intern_all::{ev, i, Tok};
use never::Never;
use substack::Substack;
use trait_set::trait_set;

use crate::error::{ProjectError, ProjectErrorObj};
use crate::location::CodeOrigin;
use crate::name::{VName, VPath};
use crate::utils::boxed_iter::BoxedIter;
use crate::utils::combine::Combine;
use crate::utils::join::try_join_maps;
use crate::utils::sequence::Sequence;

/// An umbrella trait for operations you can carry out on any part of the tree
/// structure
pub trait TreeTransforms: Sized {
  /// Data held at the leaves of the tree
  type Item;
  /// Data associated with modules
  type XMod;
  /// Data associated with entries inside modules
  type XEnt;
  /// Recursive type to enable [TreeTransforms::map_data] to transform the whole
  /// tree
  type SelfType<T, U, V>: TreeTransforms<Item = T, XMod = U, XEnt = V>;

  /// Implementation for [TreeTransforms::map_data]
  fn map_data_rec<T, U, V>(
    self,
    item: &mut impl FnMut(Substack<Tok<String>>, Self::Item) -> T,
    module: &mut impl FnMut(Substack<Tok<String>>, Self::XMod) -> U,
    entry: &mut impl FnMut(Substack<Tok<String>>, Self::XEnt) -> V,
    path: Substack<Tok<String>>,
  ) -> Self::SelfType<T, U, V>;

  /// Transform all the data in the tree without changing its structure
  fn map_data<T, U, V>(
    self,
    mut item: impl FnMut(Substack<Tok<String>>, Self::Item) -> T,
    mut module: impl FnMut(Substack<Tok<String>>, Self::XMod) -> U,
    mut entry: impl FnMut(Substack<Tok<String>>, Self::XEnt) -> V,
  ) -> Self::SelfType<T, U, V> {
    self.map_data_rec(&mut item, &mut module, &mut entry, Substack::Bottom)
  }

  /// Visit all elements in the tree. This is like [TreeTransforms::search] but
  /// without the early exit
  ///
  /// * init - can be used for reduce, otherwise pass `()`
  /// * callback - a callback applied on every module.
  ///   * [`Substack<Tok<String>>`] - the walked path
  ///   * [Module] - the current module
  ///   * `T` - data for reduce.
  fn search_all<'a, T>(
    &'a self,
    init: T,
    mut callback: impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Self::Item, Self::XMod, Self::XEnt>,
      T,
    ) -> T,
  ) -> T {
    let res =
      self.search(init, |stack, member, state| Ok::<T, Never>(callback(stack, member, state)));
    res.unwrap_or_else(|e| match e {})
  }

  /// Visit elements in the tree depth first with the provided function
  ///
  /// * init - can be used for reduce, otherwise pass `()`
  /// * callback - a callback applied on every module. Can return [Err] to
  ///   short-circuit the walk
  ///   * [`Substack<Tok<String>>`] - the walked path
  ///   * [Module] - the current module
  ///   * `T` - data for reduce.
  fn search<'a, T, E>(
    &'a self,
    init: T,
    mut callback: impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Self::Item, Self::XMod, Self::XEnt>,
      T,
    ) -> Result<T, E>,
  ) -> Result<T, E> {
    self.search_rec(init, Substack::Bottom, &mut callback)
  }

  /// Internal version of [TreeTransforms::search_all]
  fn search_rec<'a, T, E>(
    &'a self,
    state: T,
    stack: Substack<Tok<String>>,
    callback: &mut impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Self::Item, Self::XMod, Self::XEnt>,
      T,
    ) -> Result<T, E>,
  ) -> Result<T, E>;
}

/// The member in a [ModEntry] which is associated with a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModMember<Item, XMod, XEnt> {
  /// Arbitrary data
  Item(Item),
  /// A child module
  Sub(Module<Item, XMod, XEnt>),
}

impl<Item, XMod, XEnt> TreeTransforms for ModMember<Item, XMod, XEnt> {
  type Item = Item;
  type XEnt = XEnt;
  type XMod = XMod;
  type SelfType<T, U, V> = ModMember<T, U, V>;

  fn map_data_rec<T, U, V>(
    self,
    item: &mut impl FnMut(Substack<Tok<String>>, Item) -> T,
    module: &mut impl FnMut(Substack<Tok<String>>, XMod) -> U,
    entry: &mut impl FnMut(Substack<Tok<String>>, XEnt) -> V,
    path: Substack<Tok<String>>,
  ) -> Self::SelfType<T, U, V> {
    match self {
      Self::Item(it) => ModMember::Item(item(path, it)),
      Self::Sub(sub) => ModMember::Sub(sub.map_data_rec(item, module, entry, path)),
    }
  }

  fn search_rec<'a, T, E>(
    &'a self,
    state: T,
    stack: Substack<Tok<String>>,
    callback: &mut impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Item, XMod, XEnt>,
      T,
    ) -> Result<T, E>,
  ) -> Result<T, E> {
    match self {
      Self::Item(it) => callback(stack, ModMemberRef::Item(it), state),
      Self::Sub(m) => m.search_rec(state, stack, callback),
    }
  }
}

/// Reasons why merging trees might fail
pub enum ConflictKind<Item: Combine, XMod: Combine, XEnt: Combine> {
  /// Error during the merging of items
  Item(Item::Error),
  /// Error during the merging of module metadata
  Module(XMod::Error),
  /// Error during the merging of entry metadata
  XEnt(XEnt::Error),
  /// An item appeared in one tree where the other contained a submodule
  ItemModule,
}

macro_rules! impl_for_conflict {
  ($target:ty, ($($deps:tt)*), $for:ty, $body:tt) => {
    impl<Item: Combine, XMod: Combine, XEnt: Combine> $target
    for $for
    where
      Item::Error: $($deps)*,
      XMod::Error: $($deps)*,
      XEnt::Error: $($deps)*,
    $body
  };
}

impl_for_conflict!(Clone, (Clone), ConflictKind<Item, XMod, XEnt>, {
  fn clone(&self) -> Self {
    match self {
      ConflictKind::Item(it_e) => ConflictKind::Item(it_e.clone()),
      ConflictKind::Module(mod_e) => ConflictKind::Module(mod_e.clone()),
      ConflictKind::XEnt(ent_e) => ConflictKind::XEnt(ent_e.clone()),
      ConflictKind::ItemModule => ConflictKind::ItemModule,
    }
  }
});

impl_for_conflict!(fmt::Debug, (fmt::Debug), ConflictKind<Item, XMod, XEnt>, {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ConflictKind::Item(it_e) =>
        f.debug_tuple("TreeCombineErr::Item").field(it_e).finish(),
      ConflictKind::Module(mod_e) =>
        f.debug_tuple("TreeCombineErr::Module").field(mod_e).finish(),
      ConflictKind::XEnt(ent_e) =>
        f.debug_tuple("TreeCombineErr::XEnt").field(ent_e).finish(),
      ConflictKind::ItemModule => write!(f, "TreeCombineErr::Item2Module"),
    }
  }
});

/// Error produced when two trees cannot be merged
pub struct TreeConflict<Item: Combine, XMod: Combine, XEnt: Combine> {
  /// Which subtree caused the failure
  pub path: VPath,
  /// What type of failure occurred
  pub kind: ConflictKind<Item, XMod, XEnt>,
}
impl<Item: Combine, XMod: Combine, XEnt: Combine> TreeConflict<Item, XMod, XEnt> {
  fn new(kind: ConflictKind<Item, XMod, XEnt>) -> Self { Self { path: VPath::new([]), kind } }

  fn push(self, seg: Tok<String>) -> Self {
    Self { path: self.path.prefix([seg]), kind: self.kind }
  }
}

impl_for_conflict!(Clone, (Clone), TreeConflict<Item, XMod, XEnt>, {
  fn clone(&self) -> Self {
    Self { path: self.path.clone(), kind: self.kind.clone() }
  }
});

impl_for_conflict!(fmt::Debug, (fmt::Debug), TreeConflict<Item, XMod, XEnt>, {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TreeConflict")
      .field("path", &self.path)
      .field("kind", &self.kind)
      .finish()
  }
});

impl<Item: Combine, XMod: Combine, XEnt: Combine> Combine for ModMember<Item, XMod, XEnt> {
  type Error = TreeConflict<Item, XMod, XEnt>;

  fn combine(self, other: Self) -> Result<Self, Self::Error> {
    match (self, other) {
      (Self::Item(i1), Self::Item(i2)) => match i1.combine(i2) {
        Ok(i) => Ok(Self::Item(i)),
        Err(e) => Err(TreeConflict::new(ConflictKind::Item(e))),
      },
      (Self::Sub(m1), Self::Sub(m2)) => m1.combine(m2).map(Self::Sub),
      (..) => Err(TreeConflict::new(ConflictKind::ItemModule)),
    }
  }
}

/// Data about a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry<Item, XMod, XEnt> {
  /// The submodule or item
  pub member: ModMember<Item, XMod, XEnt>,
  /// Additional fields
  pub x: XEnt,
}
impl<Item: Combine, XMod: Combine, XEnt: Combine> Combine for ModEntry<Item, XMod, XEnt> {
  type Error = TreeConflict<Item, XMod, XEnt>;
  fn combine(self, other: Self) -> Result<Self, Self::Error> {
    match self.x.combine(other.x) {
      Err(e) => Err(TreeConflict::new(ConflictKind::XEnt(e))),
      Ok(x) => Ok(Self { x, member: self.member.combine(other.member)? }),
    }
  }
}
impl<Item, XMod, XEnt> ModEntry<Item, XMod, XEnt> {
  /// Returns the item in this entry if it contains one.
  #[must_use]
  pub fn item(&self) -> Option<&Item> {
    match &self.member {
      ModMember::Item(it) => Some(it),
      ModMember::Sub(_) => None,
    }
  }
}

impl<Item, XMod, XEnt> TreeTransforms for ModEntry<Item, XMod, XEnt> {
  type Item = Item;
  type XEnt = XEnt;
  type XMod = XMod;
  type SelfType<T, U, V> = ModEntry<T, U, V>;

  fn map_data_rec<T, U, V>(
    self,
    item: &mut impl FnMut(Substack<Tok<String>>, Item) -> T,
    module: &mut impl FnMut(Substack<Tok<String>>, XMod) -> U,
    entry: &mut impl FnMut(Substack<Tok<String>>, XEnt) -> V,
    path: Substack<Tok<String>>,
  ) -> Self::SelfType<T, U, V> {
    ModEntry {
      member: self.member.map_data_rec(item, module, entry, path.clone()),
      x: entry(path, self.x),
    }
  }

  fn search_rec<'a, T, E>(
    &'a self,
    state: T,
    stack: Substack<Tok<String>>,
    callback: &mut impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Item, XMod, XEnt>,
      T,
    ) -> Result<T, E>,
  ) -> Result<T, E> {
    self.member.search_rec(state, stack, callback)
  }
}
impl<Item, XMod, XEnt: Default> ModEntry<Item, XMod, XEnt> {
  /// Wrap a member directly with trivial metadata
  pub fn wrap(member: ModMember<Item, XMod, XEnt>) -> Self { Self { member, x: XEnt::default() } }
  /// Wrap an item directly with trivial metadata
  pub fn leaf(item: Item) -> Self { Self::wrap(ModMember::Item(item)) }
}
impl<Item, XMod: Default, XEnt: Default> ModEntry<Item, XMod, XEnt> {
  /// Create an empty submodule
  pub fn empty() -> Self { Self::wrap(ModMember::Sub(Module::wrap([]))) }

  /// Create a module
  #[must_use]
  pub fn tree<K: AsRef<str>>(arr: impl IntoIterator<Item = (K, Self)>) -> Self {
    Self::wrap(ModMember::Sub(Module::wrap(arr.into_iter().map(|(k, v)| (i(k.as_ref()), v)))))
  }

  /// Create a record in the list passed to [ModEntry#tree] which describes a
  /// submodule. This mostly exists to deal with strange rustfmt block
  /// breaking behaviour
  pub fn tree_ent<K: AsRef<str>>(key: K, arr: impl IntoIterator<Item = (K, Self)>) -> (K, Self) {
    (key, Self::tree(arr))
  }

  /// Namespace the tree with the list of names
  ///
  /// The unarray is used to trick rustfmt into breaking the sub-item
  /// into a block without breaking anything else.
  #[must_use]
  pub fn ns(name: impl AsRef<str>, [mut end]: [Self; 1]) -> Self {
    let elements = name.as_ref().split("::").collect::<Vec<_>>();
    for name in elements.into_iter().rev() {
      end = Self::tree([(name, end)]);
    }
    end
  }

  fn not_mod_panic<T>() -> T { panic!("Expected module but found leaf") }

  /// Return the wrapped module. Panic if the entry wraps an item
  pub fn unwrap_mod(self) -> Module<Item, XMod, XEnt> {
    if let ModMember::Sub(m) = self.member { m } else { Self::not_mod_panic() }
  }

  /// Return the wrapped module. Panic if the entry wraps an item
  pub fn unwrap_mod_ref(&self) -> &Module<Item, XMod, XEnt> {
    if let ModMember::Sub(m) = &self.member { m } else { Self::not_mod_panic() }
  }
}

/// A module, containing imports,
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<Item, XMod, XEnt> {
  /// Submodules and items by name
  pub entries: HashMap<Tok<String>, ModEntry<Item, XMod, XEnt>>,
  /// Additional fields
  pub x: XMod,
}

trait_set! {
  /// A filter applied to a module tree
  pub trait Filter<'a, Item, XMod, XEnt> =
    for<'b> Fn(&'b ModEntry<Item, XMod, XEnt>) -> bool + Clone + 'a
}

/// A line in a [Module]
pub type Record<Item, XMod, XEnt> = (Tok<String>, ModEntry<Item, XMod, XEnt>);

impl<Item, XMod, XEnt> Module<Item, XMod, XEnt> {
  /// Returns child names for which the value matches a filter
  #[must_use]
  pub fn keys<'a>(
    &'a self,
    filter: impl for<'b> Fn(&'b ModEntry<Item, XMod, XEnt>) -> bool + 'a,
  ) -> BoxedIter<Tok<String>> {
    Box::new((self.entries.iter()).filter(move |(_, v)| filter(v)).map(|(k, _)| k.clone()))
  }

  /// Return the module at the end of the given path
  pub fn walk_ref<'a: 'b, 'b>(
    &'a self,
    prefix: &'b [Tok<String>],
    path: &'b [Tok<String>],
    filter: impl Filter<'b, Item, XMod, XEnt>,
  ) -> Result<&'a Self, WalkError<'b>> {
    let mut module = self;
    for (pos, step) in path.iter().enumerate() {
      let kind = match module.entries.get(step) {
        None => ErrKind::Missing,
        Some(ent) if !filter(ent) => ErrKind::Filtered,
        Some(ModEntry { member: ModMember::Item(_), .. }) => ErrKind::NotModule,
        Some(ModEntry { member: ModMember::Sub(next), .. }) => {
          module = next;
          continue;
        },
      };
      let options = Sequence::new(move || module.keys(filter.clone()));
      return Err(WalkError { kind, prefix, path, pos, options });
    }
    Ok(module)
  }

  /// Return the member at the end of the given path
  ///
  /// # Panics
  ///
  /// if path is empty, since the reference cannot be forwarded that way
  pub fn walk1_ref<'a: 'b, 'b>(
    &'a self,
    prefix: &'b [Tok<String>],
    path: &'b [Tok<String>],
    filter: impl Filter<'b, Item, XMod, XEnt>,
  ) -> Result<(&'a ModEntry<Item, XMod, XEnt>, &'a Self), WalkError<'b>> {
    let (last, parent) = path.split_last().expect("Path cannot be empty");
    let pos = path.len() - 1;
    let module = self.walk_ref(prefix, parent, filter.clone())?;
    let err_kind = match &module.entries.get(last) {
      Some(entry) if filter(entry) => return Ok((entry, module)),
      Some(_) => ErrKind::Filtered,
      None => ErrKind::Missing,
    };
    let options = Sequence::new(move || module.keys(filter.clone()));
    Err(WalkError { kind: err_kind, options, prefix, path, pos })
  }

  /// Walk from one node to another in a tree, asserting that the origin can see
  /// the target.
  ///
  /// # Panics
  ///
  /// If the target is the root node
  pub fn inner_walk<'a: 'b, 'b>(
    &'a self,
    origin: &[Tok<String>],
    target: &'b [Tok<String>],
    is_exported: impl for<'c> Fn(&'c ModEntry<Item, XMod, XEnt>) -> bool + Clone + 'b,
  ) -> Result<(&'a ModEntry<Item, XMod, XEnt>, &'a Self), WalkError<'b>> {
    let ignore_vis_len = 1 + origin.iter().zip(target).take_while(|(a, b)| a == b).count();
    if target.len() <= ignore_vis_len {
      return self.walk1_ref(&[], target, |_| true);
    }
    let (ignore_vis_path, hidden_path) = target.split_at(ignore_vis_len);
    let first_divergence = self.walk_ref(&[], ignore_vis_path, |_| true)?;
    first_divergence.walk1_ref(ignore_vis_path, hidden_path, is_exported)
  }

  /// Wrap entry table in a module with trivial metadata
  pub fn wrap(entries: impl IntoIterator<Item = Record<Item, XMod, XEnt>>) -> Self
  where XMod: Default {
    Self { entries: entries.into_iter().collect(), x: XMod::default() }
  }
}

impl<Item, XMod, XEnt> TreeTransforms for Module<Item, XMod, XEnt> {
  type Item = Item;
  type XEnt = XEnt;
  type XMod = XMod;
  type SelfType<T, U, V> = Module<T, U, V>;

  fn map_data_rec<T, U, V>(
    self,
    item: &mut impl FnMut(Substack<Tok<String>>, Item) -> T,
    module: &mut impl FnMut(Substack<Tok<String>>, XMod) -> U,
    entry: &mut impl FnMut(Substack<Tok<String>>, XEnt) -> V,
    path: Substack<Tok<String>>,
  ) -> Self::SelfType<T, U, V> {
    Module {
      x: module(path.clone(), self.x),
      entries: (self.entries.into_iter())
        .map(|(k, e)| (k.clone(), e.map_data_rec(item, module, entry, path.push(k))))
        .collect(),
    }
  }

  fn search_rec<'a, T, E>(
    &'a self,
    mut state: T,
    stack: Substack<Tok<String>>,
    callback: &mut impl FnMut(
      Substack<Tok<String>>,
      ModMemberRef<'a, Item, XMod, XEnt>,
      T,
    ) -> Result<T, E>,
  ) -> Result<T, E> {
    state = callback(stack.clone(), ModMemberRef::Mod(self), state)?;
    for (key, value) in &self.entries {
      state = value.search_rec(state, stack.push(key.clone()), callback)?;
    }
    Ok(state)
  }
}

impl<Item: Combine, XMod: Combine, XEnt: Combine> Combine for Module<Item, XMod, XEnt> {
  type Error = TreeConflict<Item, XMod, XEnt>;
  fn combine(self, Self { entries, x }: Self) -> Result<Self, Self::Error> {
    let entries =
      try_join_maps(self.entries, entries, |k, l, r| l.combine(r).map_err(|e| e.push(k.clone())))?;
    let x = (self.x.combine(x)).map_err(|e| TreeConflict::new(ConflictKind::Module(e)))?;
    Ok(Self { x, entries })
  }
}

impl<Item: fmt::Display, TExt: fmt::Display, XEnt: fmt::Display> fmt::Display
  for Module<Item, TExt, XEnt>
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "module {{")?;
    for (name, ModEntry { member, x: extra }) in &self.entries {
      match member {
        ModMember::Sub(module) => write!(f, "\n{name} {extra} = {module}"),
        ModMember::Item(item) => write!(f, "\n{name} {extra} = {item}"),
      }?;
    }
    write!(f, "\n\n{}\n}}", &self.x)
  }
}

/// A non-owning version of [ModMember]. Either an item-ref or a module-ref.
pub enum ModMemberRef<'a, Item, XMod, XEnt> {
  /// Leaf
  Item(&'a Item),
  /// Node
  Mod(&'a Module<Item, XMod, XEnt>),
}

/// Possible causes why the path could not be walked
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrKind {
  /// `require_exported` was set to `true` and a module wasn't exported
  Filtered,
  /// A module was not found
  Missing,
  /// The path leads into a leaf node
  NotModule,
}

#[derive(Clone)]
/// All details about a failed tree-walk
pub struct WalkError<'a> {
  /// Failure mode
  kind: ErrKind,
  /// Path to the module where the walk started
  prefix: &'a [Tok<String>],
  /// Planned walk path
  path: &'a [Tok<String>],
  /// Index into walked path where the error occurred
  pos: usize,
  /// Alternatives to the failed steps
  options: Sequence<'a, Tok<String>>,
}
impl<'a> WalkError<'a> {
  /// Total length of the path represented by this error
  #[must_use]
  pub fn depth(&self) -> usize { self.prefix.len() + self.pos + 1 }

  /// Attach a location to the error and convert into trait object for reporting
  #[must_use]
  pub fn at(self, origin: &CodeOrigin) -> ProjectErrorObj {
    let details = WalkErrorDetails {
      origin: origin.clone(),
      path: VName::new((self.prefix.iter()).chain(self.path.iter().take(self.pos + 1)).cloned())
        .expect("empty paths don't cause an error"),
      options: self.options.iter().collect(),
    };
    match self.kind {
      ErrKind::Filtered => FilteredError(details).pack(),
      ErrKind::Missing => MissingError(details).pack(),
      ErrKind::NotModule => NotModuleError(details).pack(),
    }
  }
  /// Construct an error for the very last item in a slice. This is often done
  /// outside [super::tree] so it gets a function rather than exposing the
  /// fields of [WalkError]
  pub fn last(path: &'a [Tok<String>], kind: ErrKind, options: Sequence<'a, Tok<String>>) -> Self {
    WalkError { kind, path, options, pos: path.len() - 1, prefix: &[] }
  }
}
impl<'a> fmt::Debug for WalkError<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("WalkError")
      .field("kind", &self.kind)
      .field("prefix", &self.prefix)
      .field("path", &self.path)
      .field("pos", &self.pos)
      .finish_non_exhaustive()
  }
}

struct WalkErrorDetails {
  path: VName,
  options: Vec<Tok<String>>,
  origin: CodeOrigin,
}
impl WalkErrorDetails {
  fn print_options(&self) -> String { format!("options are {}", ev(&self.options).join(", ")) }
}

struct FilteredError(WalkErrorDetails);
impl ProjectError for FilteredError {
  const DESCRIPTION: &'static str = "The path leads into a private module";
  fn one_position(&self) -> CodeOrigin { self.0.origin.clone() }
  fn message(&self) -> String { format!("{} is private, {}", self.0.path, self.0.print_options()) }
}

struct MissingError(WalkErrorDetails);
impl ProjectError for MissingError {
  const DESCRIPTION: &'static str = "Nonexistent path";
  fn one_position(&self) -> CodeOrigin { self.0.origin.clone() }
  fn message(&self) -> String {
    format!("{} does not exist, {}", self.0.path, self.0.print_options())
  }
}

struct NotModuleError(WalkErrorDetails);
impl ProjectError for NotModuleError {
  const DESCRIPTION: &'static str = "The path leads into a leaf";
  fn one_position(&self) -> CodeOrigin { self.0.origin.clone() }
  fn message(&self) -> String {
    format!("{} is not a module, {}", self.0.path, self.0.print_options())
  }
}
