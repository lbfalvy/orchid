//! Generic module tree structure
//!
//! Used by various stages of the pipeline with different parameters
use std::fmt::{Debug, Display};
use std::ops::Add;
use std::rc::Rc;

use hashbrown::HashMap;

use super::Location;
use crate::error::ProjectError;
use crate::interner::Tok;
use crate::utils::substack::Substack;
use crate::utils::BoxedIter;
use crate::{Interner, VName};

/// The member in a [ModEntry] which is associated with a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModMember<TItem: Clone, TExt: Clone> {
  /// Arbitrary data
  Item(TItem),
  /// A child module
  Sub(Module<TItem, TExt>),
}

/// Data about a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry<TItem: Clone, TExt: Clone> {
  /// The submodule or item
  pub member: ModMember<TItem, TExt>,
  /// Whether the member is visible to modules other than the parent
  pub exported: bool,
}
impl<TItem: Clone, TExt: Clone> ModEntry<TItem, TExt> {
  /// Returns the item in this entry if it contains one.
  #[must_use]
  pub fn item(&self) -> Option<&TItem> {
    match &self.member {
      ModMember::Item(it) => Some(it),
      ModMember::Sub(_) => None,
    }
  }
}

/// A module, containing imports,
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<TItem: Clone, TExt: Clone> {
  /// Submodules and items by name
  pub entries: HashMap<Tok<String>, ModEntry<TItem, TExt>>,
  /// Additional information associated with the module
  pub extra: TExt,
}

/// The path taken to reach a given module
pub type ModPath<'a> = Substack<'a, Tok<String>>;

impl<TItem: Clone, TExt: Clone> Module<TItem, TExt> {
  /// If the argument is false, returns all child names.
  /// If the argument is  true, returns all public child names.
  #[must_use]
  pub fn keys(&self, public: bool) -> BoxedIter<Tok<String>> {
    match public {
      false => Box::new(self.entries.keys().cloned()),
      true => Box::new(
        (self.entries.iter())
          .filter(|(_, v)| v.exported)
          .map(|(k, _)| k.clone()),
      ),
    }
  }

  /// Return the module at the end of the given path
  pub fn walk_ref<'a: 'b, 'b>(
    &'a self,
    prefix: &'b [Tok<String>],
    path: &'b [Tok<String>],
    public: bool,
  ) -> Result<&'a Self, WalkError<'b>> {
    let mut module = self;
    for (pos, step) in path.iter().enumerate() {
      let kind = match module.entries.get(step) {
        None => ErrKind::Missing,
        Some(ModEntry { exported: false, .. }) if public => ErrKind::Private,
        Some(ModEntry { member: ModMember::Item(_), .. }) => ErrKind::NotModule,
        Some(ModEntry { member: ModMember::Sub(next), .. }) => {
          module = next;
          continue;
        },
      };
      let options = module.keys(public);
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
    public: bool,
  ) -> Result<(&'a ModEntry<TItem, TExt>, &'a Self), WalkError<'b>> {
    let (last, parent) = path.split_last().expect("Path cannot be empty");
    let pos = path.len() - 1;
    let module = self.walk_ref(prefix, parent, public)?;
    if let Some(entry) = &module.entries.get(last) {
      if !entry.exported && public {
        let options = module.keys(public);
        Err(WalkError { kind: ErrKind::Private, options, prefix, path, pos })
      } else {
        Ok((entry, module))
      }
    } else {
      let options = module.keys(public);
      Err(WalkError { kind: ErrKind::Missing, options, prefix, path, pos })
    }
  }

  fn search_all_rec<'a, T, E>(
    &'a self,
    path: ModPath,
    mut state: T,
    callback: &mut impl FnMut(ModPath, &'a Self, T) -> Result<T, E>,
  ) -> Result<T, E> {
    state = callback(path.clone(), self, state)?;
    for (name, entry) in &self.entries {
      if let ModMember::Sub(module) = &entry.member {
        state =
          module.search_all_rec(path.push(name.clone()), state, callback)?;
      }
    }
    Ok(state)
  }

  /// Visit every element in the tree with the provided function
  ///
  /// * init - can be used for reduce, otherwise pass `()`
  /// * callback - a callback applied on every module. Can return [Err] to
  ///   short-circuit the walk
  ///   * [ModPath] - a substack indicating the path to the current module from
  ///     wherever the walk begun
  ///   * [Module] - the current module
  ///   * T - data for reduce. If not used, destructure `()`
  pub fn search_all<'a, T, E>(
    &'a self,
    init: T,
    callback: &mut impl FnMut(ModPath, &'a Self, T) -> Result<T, E>,
  ) -> Result<T, E> {
    self.search_all_rec(Substack::Bottom, init, callback)
  }

  /// Combine two module trees; wherever they conflict, the overlay is
  /// preferred.
  pub fn overlay<E>(mut self, overlay: Self) -> Result<Self, E>
  where
    TExt: Add<TExt, Output = Result<TExt, E>>,
  {
    let Module { extra, entries: items } = overlay;
    let mut new_items = HashMap::new();
    for (key, right) in items {
      // if both contain a submodule
      match (self.entries.remove(&key), right) {
        (
          Some(ModEntry { member: ModMember::Sub(lsub), .. }),
          ModEntry { member: ModMember::Sub(rsub), exported },
        ) => new_items.insert(key, ModEntry {
          exported,
          member: ModMember::Sub(lsub.overlay(rsub)?),
        }),
        (_, right) => new_items.insert(key, right),
      };
    }
    new_items.extend(self.entries);
    Ok(Module { entries: new_items, extra: (self.extra + extra)? })
  }
}

impl<TItem: Clone + Display, TExt: Clone + Display> Display
  for Module<TItem, TExt>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Module {{\nchildren:")?;
    for (name, entry) in &self.entries {
      match entry.exported {
        true => write!(f, "\npublic {name} = "),
        false => write!(f, "\n{name} = "),
      }?;
      match &entry.member {
        ModMember::Sub(module) => write!(f, "{module}"),
        ModMember::Item(item) => write!(f, "{item}"),
      }?;
    }
    write!(f, "\nextra: {}\n}}", &self.extra)
  }
}

/// Possible causes why the path could not be walked
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrKind {
  /// `require_exported` was set to `true` and a module wasn't exported
  Private,
  /// A module was not found
  Missing,
  /// The path leads into a leaf node
  NotModule,
}

/// All details about a failed tree-walk
pub struct WalkError<'a> {
  /// Failure mode
  pub kind: ErrKind,
  /// Path to the module where the walk started
  pub prefix: &'a [Tok<String>],
  /// Planned walk path
  pub path: &'a [Tok<String>],
  /// Index into walked path where the error occurred
  pub pos: usize,
  /// Alternatives to the failed steps
  pub options: BoxedIter<'a, Tok<String>>,
}
impl<'a> WalkError<'a> {
  /// Total length of the path represented by this error
  #[must_use]
  pub fn depth(&self) -> usize { self.prefix.len() + self.pos + 1 }

  /// Attach a location to the error and convert into trait object for reporting
  #[must_use]
  pub fn at(self, location: &Location) -> Rc<dyn ProjectError> {
    // panic!("hello");
    WalkErrorWithLocation {
      kind: self.kind,
      location: location.clone(),
      path: (self.prefix.iter())
        .chain(self.path.iter().take(self.pos + 1))
        .cloned()
        .collect(),
      options: self.options.collect(),
    }
    .rc()
  }
}

/// Error produced by [WalkError::at]
struct WalkErrorWithLocation {
  path: VName,
  kind: ErrKind,
  options: VName,
  location: Location,
}
impl ProjectError for WalkErrorWithLocation {
  fn description(&self) -> &str {
    match self.kind {
      ErrKind::Missing => "Nonexistent path",
      ErrKind::NotModule => "The path leads into a leaf",
      ErrKind::Private => "The path leads into a private module",
    }
  }

  fn message(&self) -> String {
    let paths = Interner::extern_all(&self.path).join("::");
    let options = Interner::extern_all(&self.options).join(", ");
    match &self.kind {
      ErrKind::Missing => {
        format!("{paths} does not exist, options are {options}")
      },
      ErrKind::NotModule => {
        format!("{paths} is not a module, options are {options}")
      },
      ErrKind::Private => format!("{paths} is private, options are {options}"),
    }
  }

  fn one_position(&self) -> Location { self.location.clone() }
}
