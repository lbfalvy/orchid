use std::num::NonZero;

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
  pub fn to_api_rec(&self, vfses: &mut HashMap<api::VfsId, &'static dyn VirtFS>) -> api::EagerVfs {
    match self {
      DeclFs::Lazy(fs) => {
        let vfsc: u16 = vfses.len().try_into().expect("too many vfses (more than u16::MAX)");
        let id = api::VfsId(NonZero::new(vfsc + 1).unwrap());
        vfses.insert(id, *fs);
        api::EagerVfs::Lazy(id)
      },
      DeclFs::Mod(children) => api::EagerVfs::Eager(
        children.iter().map(|(k, v)| (intern(*k).marker(), v.to_api_rec(vfses))).collect(),
      ),
    }
  }
}
