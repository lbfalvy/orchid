use std::future::{Future, ready};
use std::num::NonZero;

use futures::FutureExt;
use futures::future::{join, join_all};
use hashbrown::HashMap;
use orchid_base::interner::intern;
use orchid_base::name::PathSlice;

use crate::api;

pub trait VirtFS: Send + Sync + 'static {
	fn load(&self, path: &PathSlice) -> api::OrcResult<api::Loaded>;
}

pub enum DeclFs {
	Lazy(&'static dyn VirtFS),
	Mod(&'static [(&'static str, DeclFs)]),
}
impl DeclFs {
	pub fn to_api_rec(
		&self,
		vfses: &mut HashMap<api::VfsId, &'static dyn VirtFS>,
	) -> impl Future<Output = api::EagerVfs> + '_ {
		match self {
			DeclFs::Lazy(fs) => {
				let vfsc: u16 = vfses.len().try_into().expect("too many vfses (more than u16::MAX)");
				let id = api::VfsId(NonZero::new(vfsc + 1).unwrap());
				vfses.insert(id, *fs);
				ready(api::EagerVfs::Lazy(id)).boxed_local()
			},
			DeclFs::Mod(children) => {
				let promises: Vec<_> =
					children.iter().map(|(k, v)| join(intern(*k), v.to_api_rec(vfses))).collect();
				async {
					api::EagerVfs::Eager(
						join_all(promises).await.into_iter().map(|(k, v)| (k.to_api(), v)).collect(),
					)
				}
				.boxed_local()
			},
		}
	}
}
