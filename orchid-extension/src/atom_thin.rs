use std::any::{type_name, Any, TypeId};
use std::fmt;
use std::io::Write;
use std::marker::PhantomData;

use orchid_api::atom::LocalAtom;
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Coding, Decode, Encode};

use crate::atom::{
  get_info, AtomCard, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant,
  ErrorNotCallable,
};
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
  fn decode(&self, mut data: &[u8]) -> Box<dyn Any> { Box::new(T::decode(&mut data)) }
  fn call(&self, buf: &[u8], ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn call_ref(&self, buf: &[u8], ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn handle_req(&self, buf: &[u8], ctx: SysCtx, req: &mut dyn std::io::Read, rep: &mut dyn Write) {
    T::decode(&mut &buf[..]).handle_req(ctx, Decode::decode(req), rep)
  }
  fn same(&self, buf: &[u8], ctx: SysCtx, buf2: &[u8]) -> bool {
    T::decode(&mut &buf[..]).same(ctx, &T::decode(&mut &buf2[..]))
  }
  fn drop(&self, buf: &[u8], _ctx: SysCtx) {
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
  fn handle_req(&self, ctx: SysCtx, req: Self::Req, rep: &mut (impl Write + ?Sized));
}
