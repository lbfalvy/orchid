use std::sync::Arc;

use hashbrown::HashMap;
use orchid_api::error::ProjResult;
use orchid_api::vfs::{EagerVfs, Loaded, VfsId};
use orchid_base::intern::{intern, Token};
use orchid_base::name::PathSlice;
use substack::Substack;
use trait_set::trait_set;

pub trait VirtFS: Send + Sync + 'static {
  fn load(&self, path: &PathSlice) -> ProjResult<Loaded>;
}

trait_set! {
  pub trait RecFsHandler<E> = FnMut(Substack<Token<String>>, &Arc<dyn VirtFS>) -> Result<(), E>;
}

pub enum DeclFs {
  Lazy(Arc<dyn VirtFS>),
  Mod(HashMap<Token<String>, DeclFs>),
}
impl DeclFs {
  pub fn module(entries: impl IntoIterator<Item = (&'static str, Self)>) -> Self {
    Self::Mod(entries.into_iter().map(|(k, v)| (intern(k), v)).collect())
  }
  fn rec<E>(&self, path: Substack<Token<String>>, f: &mut impl RecFsHandler<E>) -> Result<(), E> {
    match self {
      DeclFs::Lazy(fs) => f(path, fs),
      DeclFs::Mod(entries) => entries.iter().try_for_each(|(k, v)| v.rec(path.push(k.clone()), f)),
    }
  }
  pub fn recurse<E>(&self, f: &mut impl RecFsHandler<E>) -> Result<(), E> {
    self.rec(Substack::Bottom, f)
  }
  pub fn to_api_rec(&self, vfses: &mut HashMap<VfsId, Arc<dyn VirtFS>>) -> EagerVfs {
    match self {
      DeclFs::Lazy(fs) => {
        let id = vfses.len() as VfsId;
        vfses.insert(id, fs.clone());
        EagerVfs::Lazy(id)
      },
      DeclFs::Mod(children) => EagerVfs::Eager(
        children.into_iter().map(|(k, v)| (k.marker(), v.to_api_rec(vfses))).collect(),
      ),
    }
  }
}
