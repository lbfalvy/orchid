use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use itertools::Itertools;
use orchid_api::expr::Expr;
use orchid_api::proto::ExtMsgSet;
use orchid_api::system::{NewSystem, SysId, SystemDecl};
use orchid_api_traits::Coding;
use orchid_base::reqnot::ReqNot;
use ordered_float::NotNan;

pub struct SystemHandle<T: SystemDepCard> {
  _t: PhantomData<T>,
  id: SysId,
  reqnot: ReqNot<ExtMsgSet>,
}
impl<T: SystemDepCard> SystemHandle<T> {
  fn new(id: SysId, reqnot: ReqNot<ExtMsgSet>) -> Self { Self { _t: PhantomData, id, reqnot } }
}

pub trait System: Send {
  fn consts(&self) -> Expr;
}

pub struct SystemParams<Ctor: SystemCtor + ?Sized> {
  pub deps: <Ctor::Deps as DepSet>::Sat,
  pub id: SysId,
  pub reqnot: ReqNot<ExtMsgSet>,
}

pub trait SystemDepCard {
  type IngressReq: Coding;
  type IngressNotif: Coding;
  const NAME: &'static str;
}

pub trait DepSet {
  type Sat;
  fn report(names: &mut impl FnMut(&'static str));
  fn create(take: &mut impl FnMut() -> SysId, reqnot: ReqNot<ExtMsgSet>) -> Self::Sat;
}

impl<T: SystemDepCard> DepSet for T {
  type Sat = SystemHandle<Self>;
  fn report(names: &mut impl FnMut(&'static str)) { names(T::NAME) }
  fn create(take: &mut impl FnMut() -> SysId, reqnot: ReqNot<ExtMsgSet>) -> Self::Sat {
    SystemHandle::new(take(), reqnot)
  }
}

pub trait SystemCtor: Send {
  type Deps: DepSet;
  const NAME: &'static str;
  const VERSION: f64;
  #[allow(clippy::new_ret_no_self)]
  fn new(params: SystemParams<Self>) -> Box<dyn System>;
}

pub trait DynSystemCtor: Send {
  fn decl(&self) -> SystemDecl;
  fn new_system(&self, new: &NewSystem, reqnot: ReqNot<ExtMsgSet>) -> Box<dyn System>;
}

impl<T: SystemCtor + 'static> DynSystemCtor for T {
  fn decl(&self) -> SystemDecl {
    // Version is equivalent to priority for all practical purposes
    let priority = NotNan::new(T::VERSION).unwrap();
    // aggregate depends names
    let mut depends = Vec::new();
    T::Deps::report(&mut |n| depends.push(n.to_string()));
    // generate definitely unique id
    let mut ahash = ahash::AHasher::default();
    TypeId::of::<T>().hash(&mut ahash);
    let id = (ahash.finish().to_be_bytes().into_iter().tuples())
      .map(|(l, b)| u16::from_be_bytes([l, b]))
      .fold(0, |a, b| a ^ b);
    SystemDecl { name: T::NAME.to_string(), depends, id, priority }
  }
  fn new_system(&self, new: &NewSystem, reqnot: ReqNot<ExtMsgSet>) -> Box<dyn System> {
    let mut ids = new.depends.iter().copied();
    T::new(SystemParams {
      deps: T::Deps::create(&mut || ids.next().unwrap(), reqnot.clone()),
      id: new.id,
      reqnot,
    })
  }
}

pub struct ExtensionData {
  pub systems: Vec<Box<dyn DynSystemCtor>>,
}

mod dep_set_tuple_impls {
  use orchid_api::proto::ExtMsgSet;
  use orchid_api::system::SysId;
  use orchid_base::reqnot::ReqNot;

  use super::DepSet;

  macro_rules! dep_set_tuple_impl {
    ($($name:ident),*) => {
      impl<$( $name :DepSet ),*> DepSet for ( $( $name , )* ) {
        type Sat = ( $( $name ::Sat , )* );
        fn report(names: &mut impl FnMut(&'static str)) {
          $(
            $name ::report(names);
          )*
        }
        fn create(take: &mut impl FnMut() -> SysId, reqnot: ReqNot<ExtMsgSet>) -> Self::Sat {
          (
            $(
              $name ::create(take, reqnot.clone()),
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
