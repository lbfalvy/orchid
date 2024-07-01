//! Abstractions for handling various code-related errors under a common trait
//! object.

use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;
use std::{fmt, iter};

use dyn_clone::{clone_box, DynClone};
use itertools::Itertools;
use orchid_api::error::{ProjErr, ProjErrLocation};

use crate::boxed_iter::{box_once, BoxedIter};
use crate::intern::{deintern, intern, Token};
use crate::location::{GetSrc, Position};
#[allow(unused)] // for doc
use crate::virt_fs::CodeNotFound;
