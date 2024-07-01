use std::any::Any;
use std::num::NonZeroU16;
use std::sync::Arc;

use orchid_api::system::{NewSystem, SysId, SystemDecl};
use orchid_base::boxed_iter::{box_empty, box_once, BoxedIter};
use ordered_float::NotNan;

use crate::other_system::{DynSystemHandle, SystemHandle};
use crate::system::{DynSystem, System, SystemCard};

pub struct Cted<Ctor: SystemCtor + ?Sized> {
  pub deps: <Ctor::Deps as DepDef>::Sat,
  pub inst: Arc<Ctor::Instance>,
}
impl<C: SystemCtor + ?Sized> Clone for Cted<C> {
  fn clone(&self) -> Self { Self { deps: self.deps.clone(), inst: self.inst.clone() } }
}
pub trait DynCted: Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
  fn deps<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)>;
  fn inst(&self) -> Arc<dyn DynSystem>;
}
impl<C: SystemCtor + ?Sized> DynCted for Cted<C> {
  fn as_any(&self) -> &dyn Any { self }
  fn deps<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)> { self.deps.iter() }
  fn inst(&self) -> Arc<dyn DynSystem> { self.inst.clone() }
}
pub type CtedObj = Arc<dyn DynCted>;

pub trait DepSat: Clone + Send + Sync + 'static {
  fn iter<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)>;
}

pub trait DepDef {
  type Sat: DepSat;
  fn report(names: &mut impl FnMut(&'static str));
  fn create(take: &mut impl FnMut() -> SysId) -> Self::Sat;
}

impl<T: SystemCard> DepSat for SystemHandle<T> {
  fn iter<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)> { box_once(self) }
}

impl<T: SystemCard> DepDef for T {
  type Sat = SystemHandle<Self>;
  fn report(names: &mut impl FnMut(&'static str)) { names(T::Ctor::NAME) }
  fn create(take: &mut impl FnMut() -> SysId) -> Self::Sat { SystemHandle::new(take()) }
}

impl DepSat for () {
  fn iter<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)> { box_empty() }
}

impl DepDef for () {
  type Sat = ();
  fn create(_: &mut impl FnMut() -> SysId) -> Self::Sat {}
  fn report(_: &mut impl FnMut(&'static str)) {}
}

pub trait SystemCtor: Send + Sync + 'static {
  type Deps: DepDef;
  type Instance: System;
  const NAME: &'static str;
  const VERSION: f64;
  fn inst() -> Option<Self::Instance>;
}

pub trait DynSystemCtor: Send + Sync + 'static {
  fn decl(&self, id: NonZeroU16) -> SystemDecl;
  fn new_system(&self, new: &NewSystem) -> CtedObj;
}

impl<T: SystemCtor> DynSystemCtor for T {
  fn decl(&self, id: NonZeroU16) -> SystemDecl {
    // Version is equivalent to priority for all practical purposes
    let priority = NotNan::new(T::VERSION).unwrap();
    // aggregate depends names
    let mut depends = Vec::new();
    T::Deps::report(&mut |n| depends.push(n.to_string()));
    SystemDecl { name: T::NAME.to_string(), depends, id, priority }
  }
  fn new_system(&self, NewSystem { system: _, id: _, depends }: &NewSystem) -> CtedObj {
    let mut ids = depends.iter().copied();
    let inst = Arc::new(T::inst().expect("Constructor did not create system"));
    let deps = T::Deps::create(&mut || ids.next().unwrap());
    Arc::new(Cted::<T> { deps, inst })
  }
}

mod dep_set_tuple_impls {
  use orchid_api::system::SysId;
  use orchid_base::box_chain;
  use orchid_base::boxed_iter::BoxedIter;
  use paste::paste;

  use super::{DepDef, DepSat};
  use crate::system_ctor::DynSystemHandle;

  macro_rules! dep_set_tuple_impl {
    ($($name:ident),*) => {
      impl<$( $name :DepSat ),*> DepSat for ( $( $name , )* ) {
        fn iter<'a>(&'a self) -> BoxedIter<'a, &'a (dyn DynSystemHandle + 'a)> {
          // we're using the Paste crate to convert the names to lowercase,
          // so `dep_set_tuple_impl!(A, B, C)` generates `let (a, b, c,) = self;`
          // This step isn't really required for correctness, but Rust warns about uppercase
          // variable names.
          paste!{
            let (
              $(
                [< $name :lower >] ,
              )*
            ) = self;
            box_chain! (
              $(
                [< $name :lower >] .iter()
              ),*
            )
          }
        }
      }

      impl<$( $name :DepDef ),*> DepDef for ( $( $name , )* ) {
        type Sat = ( $( $name ::Sat , )* );
        fn report(names: &mut impl FnMut(&'static str)) {
          $(
            $name ::report(names);
          )*
        }
        fn create(take: &mut impl FnMut() -> SysId) -> Self::Sat {
          (
            $(
              $name ::create(take),
            )*
          )
        }
      }
    };
  }

  dep_set_tuple_impl!(A);
  dep_set_tuple_impl!(A, B); // 2
  dep_set_tuple_impl!(A, B, C);
  dep_set_tuple_impl!(A, B, C, D); // 4
  dep_set_tuple_impl!(A, B, C, D, E);
  dep_set_tuple_impl!(A, B, C, D, E, F);
  dep_set_tuple_impl!(A, B, C, D, E, F, G);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H); // 8
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L); // 12
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
  dep_set_tuple_impl!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P); // 16
}
