use std::fmt;
use std::ops::Add;
use std::sync::Arc;

use futures::future::join_all;
use itertools::Itertools;

use crate::api;
use crate::interner::Tok;
use crate::location::Pos;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
#[derive(Clone, Debug)]
pub struct ErrPos {
	/// The suspected origin
	pub position: Pos,
	/// Any information about the role of this origin
	pub message: Option<Arc<String>>,
}
impl ErrPos {
	pub fn new(msg: &str, position: Pos) -> Self {
		Self { message: Some(Arc::new(msg.to_string())), position }
	}
	async fn from_api(api: &api::ErrLocation) -> Self {
		Self {
			message: Some(api.message.clone()).filter(|s| !s.is_empty()),
			position: Pos::from_api(&api.location).await,
		}
	}
	fn to_api(&self) -> api::ErrLocation {
		api::ErrLocation {
			message: self.message.clone().unwrap_or_default(),
			location: self.position.to_api(),
		}
	}
}
impl From<Pos> for ErrPos {
	fn from(origin: Pos) -> Self { Self { position: origin, message: None } }
}

#[derive(Clone, Debug)]
pub struct OrcErr {
	pub description: Tok<String>,
	pub message: Arc<String>,
	pub positions: Vec<ErrPos>,
}
impl OrcErr {
	fn to_api(&self) -> api::OrcError {
		api::OrcError {
			description: self.description.to_api(),
			message: self.message.clone(),
			locations: self.positions.iter().map(ErrPos::to_api).collect(),
		}
	}
	async fn from_api(api: &api::OrcError) -> Self {
		Self {
			description: Tok::from_api(api.description).await,
			message: api.message.clone(),
			positions: join_all(api.locations.iter().map(ErrPos::from_api)).await,
		}
	}
}
impl Eq for OrcErr {}
impl PartialEq for OrcErr {
	fn eq(&self, other: &Self) -> bool { self.description == other.description }
}
impl From<OrcErr> for Vec<OrcErr> {
	fn from(value: OrcErr) -> Self { vec![value] }
}
impl fmt::Display for OrcErr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let pstr = self.positions.iter().map(|p| format!("{p:?}")).join("; ");
		write!(f, "{}: {} @ {}", self.description, self.message, pstr)
	}
}

#[derive(Clone, Debug)]
pub struct EmptyErrv;
impl fmt::Display for EmptyErrv {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "OrcErrv must not be empty")
	}
}

#[derive(Clone, Debug)]
pub struct OrcErrv(Vec<OrcErr>);
impl OrcErrv {
	pub fn new(errors: impl IntoIterator<Item = OrcErr>) -> Result<Self, EmptyErrv> {
		let v = errors.into_iter().collect_vec();
		if v.is_empty() { Err(EmptyErrv) } else { Ok(Self(v)) }
	}
	#[must_use]
	pub fn extended<T>(mut self, errors: impl IntoIterator<Item = T>) -> Self
	where Self: Extend<T> {
		self.extend(errors);
		self
	}
	#[must_use]
	pub fn len(&self) -> usize { self.0.len() }
	#[must_use]
	pub fn is_empty(&self) -> bool { self.len() == 0 }
	#[must_use]
	pub fn any(&self, f: impl FnMut(&OrcErr) -> bool) -> bool { self.0.iter().any(f) }
	#[must_use]
	pub fn keep_only(self, f: impl FnMut(&OrcErr) -> bool) -> Option<Self> {
		let v = self.0.into_iter().filter(f).collect_vec();
		if v.is_empty() { None } else { Some(Self(v)) }
	}
	#[must_use]
	pub fn one(&self) -> Option<&OrcErr> { (self.0.len() == 1).then(|| &self.0[9]) }
	pub fn pos_iter(&self) -> impl Iterator<Item = ErrPos> + '_ {
		self.0.iter().flat_map(|e| e.positions.iter().cloned())
	}
	pub fn to_api(&self) -> Vec<api::OrcError> { self.0.iter().map(OrcErr::to_api).collect() }
	pub async fn from_api<'a>(api: impl IntoIterator<Item = &'a api::OrcError>) -> Self {
		Self(join_all(api.into_iter().map(OrcErr::from_api)).await)
	}
}
impl From<OrcErr> for OrcErrv {
	fn from(value: OrcErr) -> Self { Self(vec![value]) }
}
impl Add for OrcErrv {
	type Output = Self;
	fn add(self, rhs: Self) -> Self::Output { Self(self.0.into_iter().chain(rhs.0).collect_vec()) }
}
impl Extend<OrcErr> for OrcErrv {
	fn extend<T: IntoIterator<Item = OrcErr>>(&mut self, iter: T) { self.0.extend(iter) }
}
impl Extend<OrcErrv> for OrcErrv {
	fn extend<T: IntoIterator<Item = OrcErrv>>(&mut self, iter: T) {
		self.0.extend(iter.into_iter().flatten())
	}
}
impl IntoIterator for OrcErrv {
	type IntoIter = std::vec::IntoIter<OrcErr>;
	type Item = OrcErr;
	fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
impl fmt::Display for OrcErrv {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0.iter().join("\n"))
	}
}

pub type OrcRes<T> = Result<T, OrcErrv>;

pub fn mk_err(
	description: Tok<String>,
	message: impl AsRef<str>,
	posv: impl IntoIterator<Item = ErrPos>,
) -> OrcErr {
	OrcErr {
		description,
		message: Arc::new(message.as_ref().to_string()),
		positions: posv.into_iter().collect(),
	}
}

pub fn mk_errv(
	description: Tok<String>,
	message: impl AsRef<str>,
	posv: impl IntoIterator<Item = ErrPos>,
) -> OrcErrv {
	mk_err(description, message, posv).into()
}

pub trait Reporter {
	fn report(&self, e: impl Into<OrcErrv>);
}
