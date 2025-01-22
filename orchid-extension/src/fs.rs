use std::num::NonZero;

use futures::FutureExt;
use futures::future::LocalBoxFuture;
use hashbrown::HashMap;
use orchid_base::interner::Interner;
use orchid_base::name::PathSlice;

use crate::api;
use crate::system::SysCtx;

pub trait VirtFS: Send + Sync + 'static {
	fn load<'a>(
		&'a self,
		path: &'a PathSlice,
		ctx: SysCtx,
	) -> LocalBoxFuture<'a, api::OrcResult<api::Loaded>>;
}

pub enum DeclFs {
	Lazy(&'static dyn VirtFS),
	Mod(&'static [(&'static str, DeclFs)]),
}
impl DeclFs {
	pub async fn to_api_rec(
		&self,
		vfses: &mut HashMap<api::VfsId, &'static dyn VirtFS>,
		i: &Interner,
	) -> api::EagerVfs {
		match self {
			DeclFs::Lazy(fs) => {
				let vfsc: u16 = vfses.len().try_into().expect("too many vfses (more than u16::MAX)");
				let id = api::VfsId(NonZero::new(vfsc + 1).unwrap());
				vfses.insert(id, *fs);
				api::EagerVfs::Lazy(id)
			},
			DeclFs::Mod(children) => {
				let mut output = std::collections::HashMap::new();
				for (k, v) in children.iter() {
					output
						.insert(i.i::<String>(*k).await.to_api(), v.to_api_rec(vfses, i).boxed_local().await);
				}
				api::EagerVfs::Eager(output)
			},
		}
	}
}
