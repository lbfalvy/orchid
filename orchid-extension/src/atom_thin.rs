use std::{any::type_name, fmt, io::Write};

use orchid_api::atom::LocalAtom;
use orchid_api_traits::{Coding, Decode, Encode};
use typeid::ConstTypeId;

use crate::{atom::{get_info, AtomCard, AtomFactory, AtomInfo, Atomic, AtomicFeaturesImpl, AtomicVariant, ErrorNotCallable}, expr::{bot, ExprHandle, GenExpr}, system::SysCtx};

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
  fn _info() -> &'static AtomInfo {
    &const {
      AtomInfo {
        tid: ConstTypeId::of::<Self>(),
        decode: |mut b| Box::new(Self::decode(&mut b)),
        call: |mut b, ctx, extk| Self::decode(&mut b).call(ExprHandle::from_args(ctx, extk)),
        call_ref: |mut b, ctx, extk| Self::decode(&mut b).call(ExprHandle::from_args(ctx, extk)),
        handle_req: |mut b, ctx, req, rep| Self::decode(&mut b).handle_req(ctx, Decode::decode(req), rep),
        same: |mut b1, ctx, mut b2| Self::decode(&mut b1).same(ctx, &Self::decode(&mut b2)),
        drop: |mut b1, _| eprintln!("Received drop signal for non-drop atom {:?}", Self::decode(&mut b1)),
      }
    }
  }
}

pub trait ThinAtom: AtomCard<Data = Self> + Coding + fmt::Debug {
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
