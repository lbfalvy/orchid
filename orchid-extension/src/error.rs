use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use std::{fmt, iter};

use dyn_clone::{clone_box, DynClone};
use itertools::Itertools;
use orchid_base::boxed_iter::{box_once, BoxedIter};
use orchid_base::clone;
use orchid_base::error::{ErrPos, OrcError};
use orchid_base::interner::{deintern, intern};
use orchid_base::location::{GetSrc, Pos};
use orchid_base::reqnot::{ReqNot, Requester};

use crate::api;

/// Errors addressed to the developer which are to be resolved with
/// code changes
pub trait ProjectError: Sized + Send + Sync + 'static {
  /// A general description of this type of error
  const DESCRIPTION: &'static str;
  /// A formatted message that includes specific parameters
  #[must_use]
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error. If you don't implement this, you
  /// must implement [ProjectError::one_position]
  #[must_use]
  fn positions(&self) -> impl IntoIterator<Item = ErrPos> + '_ {
    box_once(ErrPos { position: self.one_position(), message: None })
  }
  /// Short way to provide a single origin. If you don't implement this, you
  /// must implement [ProjectError::positions]
  #[must_use]
  fn one_position(&self) -> Pos {
    unimplemented!("Error type did not implement either positions or one_position")
  }
  /// Convert the error into an `Arc<dyn DynProjectError>` to be able to
  /// handle various errors together
  #[must_use]
  fn pack(self) -> ProjectErrorObj { Arc::new(self) }
}

/// Object-safe version of [ProjectError]. Implement that instead of this.
pub trait DynProjectError: Send + Sync + 'static {
  /// Access type information about this error
  #[must_use]
  fn as_any_ref(&self) -> &dyn Any;
  /// Pack the error into a trait object, or leave it as-is if it's already a
  /// trait object
  #[must_use]
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj;
  /// A general description of this type of error
  #[must_use]
  fn description(&self) -> Cow<'_, str>;
  /// A formatted message that includes specific parameters
  #[must_use]
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error.
  #[must_use]
  fn positions(&self) -> BoxedIter<'_, ErrPos>;
}

impl<T> DynProjectError for T
where T: ProjectError
{
  fn as_any_ref(&self) -> &dyn Any { self }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { self }
  fn description(&self) -> Cow<'_, str> { Cow::Borrowed(T::DESCRIPTION) }
  fn message(&self) -> String { ProjectError::message(self) }
  fn positions(&self) -> BoxedIter<ErrPos> { Box::new(ProjectError::positions(self).into_iter()) }
}

pub fn pretty_print(err: &dyn DynProjectError, get_src: &mut impl GetSrc) -> String {
  let description = err.description();
  let message = err.message();
  let positions = err.positions().collect::<Vec<_>>();
  let head = format!("Project error: {description}\n{message}");
  if positions.is_empty() {
    head + "No origins specified"
  } else {
    iter::once(head)
      .chain(positions.iter().map(|ErrPos { position: origin, message }| match message {
        None => format!("@{}", origin.pretty_print(get_src)),
        Some(msg) => format!("@{}: {msg}", origin.pretty_print(get_src)),
      }))
      .join("\n")
  }
}

impl DynProjectError for ProjectErrorObj {
  fn as_any_ref(&self) -> &dyn Any { (**self).as_any_ref() }
  fn description(&self) -> Cow<'_, str> { (**self).description() }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { (*self).clone() }
  fn message(&self) -> String { (**self).message() }
  fn positions(&self) -> BoxedIter<'_, ErrPos> { (**self).positions() }
}

/// Type-erased [ProjectError] implementor through the [DynProjectError]
/// object-trait
pub type ProjectErrorObj = Arc<dyn DynProjectError>;
/// Alias for a result with an error of [ProjectErrorObj]. This is the type of
/// result most commonly returned by pre-runtime operations.
pub type ProjectResult<T> = Result<T, ProjectErrorObj>;

/// A trait for error types that are only missing an origin. Do not depend on
/// this trait, refer to [DynErrorSansOrigin] instead.
pub trait ErrorSansOrigin: Clone + Sized + Send + Sync + 'static {
  /// General description of the error condition
  const DESCRIPTION: &'static str;
  /// Specific description of the error including code fragments or concrete
  /// data if possible
  fn message(&self) -> String { Self::DESCRIPTION.to_string() }
  /// Convert the error to a type-erased structure for handling on shared
  /// channels
  fn pack(self) -> ErrorSansOriginObj { Box::new(self) }
  /// A shortcut to streamline switching code between [ErrorSansOriginObj] and
  /// concrete types
  fn bundle(self, origin: &Pos) -> ProjectErrorObj { self.pack().bundle(origin) }
}

/// Object-safe equivalent to [ErrorSansOrigin]. Implement that one instead of
/// this. Typically found as [ErrorSansOriginObj]
pub trait DynErrorSansOrigin: Any + Send + Sync + DynClone {
  /// Allow to downcast the base object to distinguish between various errors.
  /// The main intended purpose is to trigger a fallback when [CodeNotFound] is
  /// encountered, but the possibilities are not limited to that.
  fn as_any_ref(&self) -> &dyn Any;
  /// Regularize the type
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj;
  /// Generic description of the error condition
  fn description(&self) -> Cow<'_, str>;
  /// Specific description of this particular error
  fn message(&self) -> String;
  /// Add an origin
  fn bundle(self: Box<Self>, origin: &Pos) -> ProjectErrorObj;
}

/// Type-erased [ErrorSansOrigin] implementor through the object-trait
/// [DynErrorSansOrigin]. This can be turned into a [ProjectErrorObj] with
/// [ErrorSansOriginObj::bundle].
pub type ErrorSansOriginObj = Box<dyn DynErrorSansOrigin>;
/// A generic project result without origin
pub type ResultSansOrigin<T> = Result<T, ErrorSansOriginObj>;

impl<T: ErrorSansOrigin + 'static> DynErrorSansOrigin for T {
  fn description(&self) -> Cow<'_, str> { Cow::Borrowed(Self::DESCRIPTION) }
  fn message(&self) -> String { (*self).message() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj { (*self).pack() }
  fn bundle(self: Box<Self>, origin: &Pos) -> ProjectErrorObj {
    Arc::new(OriginBundle(origin.clone(), *self))
  }
}
impl Clone for ErrorSansOriginObj {
  fn clone(&self) -> Self { clone_box(&**self) }
}
impl DynErrorSansOrigin for ErrorSansOriginObj {
  fn description(&self) -> Cow<'_, str> { (**self).description() }
  fn message(&self) -> String { (**self).message() }
  fn as_any_ref(&self) -> &dyn Any { (**self).as_any_ref() }
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj { *self }
  fn bundle(self: Box<Self>, origin: &Pos) -> ProjectErrorObj { (*self).bundle(origin) }
}
impl fmt::Display for ErrorSansOriginObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\nOrigin missing from error", self.message())
  }
}
impl fmt::Debug for ErrorSansOriginObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self}") }
}

struct OriginBundle<T: ErrorSansOrigin>(Pos, T);
impl<T: ErrorSansOrigin> DynProjectError for OriginBundle<T> {
  fn as_any_ref(&self) -> &dyn Any { self.1.as_any_ref() }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { self }
  fn description(&self) -> Cow<'_, str> { self.1.description() }
  fn message(&self) -> String { self.1.message() }
  fn positions(&self) -> BoxedIter<ErrPos> {
    box_once(ErrPos { position: self.0.clone(), message: None })
  }
}

/// A collection for tracking fatal errors without halting. Participating
/// functions return [ProjectResult] even if they only ever construct [Ok]. When
/// they call other participating functions, instead of directly forwarding
/// errors with `?` they should prefer constructing a fallback value with
/// [Reporter::fallback]. If any error is added to a [Reporter] in a function,
/// the return value is valid but its meaning need not be related in any way to
/// the inputs.
///
/// Returning [Err] from a function that accepts `&mut Reporter` indicates not
/// that there was a fatal error but that it wasn't possible to construct a
/// fallback, so if it can, the caller should construct one.
pub struct Reporter(RefCell<Vec<ProjectErrorObj>>);
impl Reporter {
  /// Create a new error reporter
  pub fn new() -> Self { Self(RefCell::new(Vec::new())) }
  /// Returns true if any errors were regorded. If this ever returns true, it
  /// will always return true in the future.
  pub fn failing(&self) -> bool { !self.0.borrow().is_empty() }
  /// Report a fatal error
  pub fn report(&self, error: ProjectErrorObj) {
    match error.as_any_ref().downcast_ref::<MultiError>() {
      None => self.0.borrow_mut().push(error),
      Some(me) =>
        for err in me.0.iter() {
          self.report(err.clone())
        },
    }
  }
  /// Catch a fatal error, report it, and substitute the value
  pub fn fallback<T>(&self, res: ProjectResult<T>, cb: impl FnOnce(ProjectErrorObj) -> T) -> T {
    res.inspect_err(|e| self.report(e.clone())).unwrap_or_else(cb)
  }
  /// Take the errors out of the reporter
  #[must_use]
  pub fn into_errors(self) -> Option<Vec<ProjectErrorObj>> {
    let v = self.0.into_inner();
    if v.is_empty() { None } else { Some(v) }
  }
  /// Raise an error if the reporter contains any errors
  pub fn bind(self) -> ProjectResult<()> {
    match self.into_errors() {
      None => Ok(()),
      Some(v) if v.len() == 1 => Err(v.into_iter().next().unwrap()),
      Some(v) => Err(MultiError(v).pack()),
    }
  }
}

impl Default for Reporter {
  fn default() -> Self { Self::new() }
}

fn unpack_into(err: impl DynProjectError, res: &mut Vec<ProjectErrorObj>) {
  match err.as_any_ref().downcast_ref::<MultiError>() {
    Some(multi) => multi.0.iter().for_each(|e| unpack_into(e.clone(), res)),
    None => res.push(Arc::new(err).into_packed()),
  }
}

pub fn unpack_err(err: ProjectErrorObj) -> Vec<ProjectErrorObj> {
  let mut out = Vec::new();
  unpack_into(err, &mut out);
  out
}

pub fn pack_err<E: DynProjectError>(iter: impl IntoIterator<Item = E>) -> ProjectErrorObj {
  let mut errors = Vec::new();
  iter.into_iter().for_each(|e| unpack_into(e, &mut errors));
  if errors.len() == 1 { errors.into_iter().next().unwrap() } else { MultiError(errors).pack() }
}

struct MultiError(Vec<ProjectErrorObj>);
impl ProjectError for MultiError {
  const DESCRIPTION: &'static str = "Multiple errors occurred";
  fn message(&self) -> String { format!("{} errors occurred", self.0.len()) }
  fn positions(&self) -> impl IntoIterator<Item = ErrPos> + '_ {
    self.0.iter().flat_map(|e| {
      e.positions().map(|pos| {
        let emsg = e.message();
        let msg = match pos.message {
          None => emsg,
          Some(s) if s.is_empty() => emsg,
          Some(pmsg) => format!("{emsg}: {pmsg}"),
        };
        ErrPos { position: pos.position, message: Some(Arc::new(msg)) }
      })
    })
  }
}

fn err_to_api(err: ProjectErrorObj) -> api::OrcErr {
  api::OrcErr {
    description: intern(&*err.description()).marker(),
    message: Arc::new(err.message()),
    locations: err.positions().map(|e| e.to_api()).collect_vec(),
  }
}

struct RelayedError {
  pub id: Option<api::ErrId>,
  pub reqnot: ReqNot<api::ExtMsgSet>,
  pub details: OnceLock<OrcError>,
}
impl RelayedError {
  fn details(&self) -> &OrcError {
    let Self { id, reqnot, details: data } = self;
    data.get_or_init(clone!(reqnot; move || {
      let id = id.expect("Either data or ID must be initialized");
      let projerr = reqnot.request(api::GetErrorDetails(id));
      OrcError {
        description: deintern(projerr.description),
        message: projerr.message,
        positions: projerr.locations.iter().map(ErrPos::from_api).collect_vec(),
      }
    }))
  }
}
impl DynProjectError for RelayedError {
  fn description(&self) -> Cow<'_, str> { Cow::Borrowed(self.details().description.as_str()) }
  fn message(&self) -> String { self.details().message.to_string() }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { self }
  fn positions(&self) -> BoxedIter<'_, ErrPos> {
    Box::new(self.details().positions.iter().cloned())
  }
}
