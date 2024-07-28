use std::any::{type_name, Any, TypeId};
use std::fmt;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;

use orchid_api::atom::LocalAtom;
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Coding, Decode, Encode};

use crate::atom::{
  get_info, AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant,
  ErrorNotCallable, ReqPck, RequestPack,
};
use crate::error::ProjectResult;
use crate::expr::{bot, ExprHandle, GenExpr};
use crate::system::SysCtx;

pub struct ThinVariant;
impl AtomicVariant for ThinVariant {}
impl<A: ThinAtom + Atomic<Variant = ThinVariant>> AtomicFeaturesImpl<ThinVariant> for A {
  fn _factory(self) -> AtomFactory {
    AtomFactory::new(move |sys| {
      let mut buf = get_info::<A>(sys.dyn_card()).0.enc_vec();
      self.encode(&mut buf);
      LocalAtom { drop: false, data: buf }
    })
  }
  type _Info = ThinAtomDynfo<Self>;
  const _INFO: &'static Self::_Info = &ThinAtomDynfo(PhantomData);
}

pub struct ThinAtomDynfo<T: ThinAtom>(PhantomData<T>);
impl<T: ThinAtom> AtomDynfo for ThinAtomDynfo<T> {
  fn tid(&self) -> TypeId { TypeId::of::<T>() }
  fn name(&self) -> &'static str { type_name::<T>() }
  fn decode(&self, AtomCtx(data, _): AtomCtx) -> Box<dyn Any> {
    Box::new(T::decode(&mut &data[..]))
  }
  fn call(&self, AtomCtx(buf, ctx): AtomCtx, arg: ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn call_ref(&self, AtomCtx(buf, ctx): AtomCtx, arg: ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn handle_req(
    &self,
    AtomCtx(buf, ctx): AtomCtx,
    req: &mut dyn std::io::Read,
    rep: &mut dyn Write,
  ) {
    T::decode(&mut &buf[..]).handle_req(ctx, RequestPack::<T, dyn Write>(Decode::decode(req), rep))
  }
  fn same(&self, AtomCtx(buf, ctx): AtomCtx, buf2: &[u8]) -> bool {
    T::decode(&mut &buf[..]).same(ctx, &T::decode(&mut &buf2[..]))
  }
  fn command(&self, AtomCtx(buf, ctx): AtomCtx<'_>) -> ProjectResult<Option<GenExpr>> {
    T::decode(&mut &buf[..]).command(ctx)
  }
  fn drop(&self, AtomCtx(buf, _): AtomCtx) {
    eprintln!("Received drop signal for non-drop atom {:?}", T::decode(&mut &buf[..]))
  }
}

pub trait ThinAtom: AtomCard<Data = Self> + Coding + fmt::Debug + Send + Sync + 'static {
  #[allow(unused_variables)]
  fn call(&self, arg: ExprHandle) -> GenExpr { bot(ErrorNotCallable) }
  #[allow(unused_variables)]
  fn same(&self, ctx: SysCtx, other: &Self) -> bool {
    eprintln!(
      "Override ThinAtom::same for {} if it can be generated during parsing",
      type_name::<Self>()
    );
    false
  }
  fn handle_req(&self, ctx: SysCtx, pck: impl ReqPck<Self>);
  #[allow(unused_variables)]
  fn command(&self, ctx: SysCtx) -> ProjectResult<Option<GenExpr>> {
    Err(Arc::new(ErrorNotCallable))
  }
}
