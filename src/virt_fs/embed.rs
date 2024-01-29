use std::sync::Arc;

use intern_all::{ev, i, Tok};
use rust_embed::RustEmbed;

use super::common::CodeNotFound;
use super::{FSResult, Loaded, VirtFS};
use crate::error::ErrorSansLocation;
use crate::location::CodeGenInfo;
use crate::name::PathSlice;
use crate::tree::{ModEntry, ModMember, Module};

/// An in-memory FS tree for libraries managed internally by the interpreter
pub struct EmbeddedFS {
  tree: Module<Arc<String>, (), ()>,
  suffix: &'static str,
  gen: CodeGenInfo,
}
impl EmbeddedFS {
  /// Expose a directory embedded in a binary wiht [RustEmbed] to the
  /// interpreter
  pub fn new<T: RustEmbed>(suffix: &'static str, gen: CodeGenInfo) -> Self {
    let mut tree = Module::wrap([]);
    for path in T::iter() {
      let data_buf = T::get(&path).expect("path from iterator").data.to_vec();
      let data = String::from_utf8(data_buf).expect("embed must be utf8");
      let mut cur_node = &mut tree;
      let path_no_suffix =
        path.strip_suffix(suffix).expect("embed filtered for suffix");
      let mut segments = path_no_suffix.split('/').map(i);
      let mut cur_seg = segments.next().expect("Embed is a directory");
      for next_seg in segments {
        if !cur_node.entries.contains_key(&cur_seg) {
          let ent = ModEntry::wrap(ModMember::Sub(Module::wrap([])));
          cur_node.entries.insert(cur_seg.clone(), ent);
        }
        let ent = cur_node.entries.get_mut(&cur_seg).expect("just constructed");
        match &mut ent.member {
          ModMember::Sub(submod) => cur_node = submod,
          _ => panic!("Aliased file and folder"),
        };
        cur_seg = next_seg;
      }
      let data_ent = ModEntry::wrap(ModMember::Item(Arc::new(data)));
      let prev = cur_node.entries.insert(cur_seg, data_ent);
      debug_assert!(prev.is_none(), "file name unique");
    }
    // if gen.generator == "std" {
    //   panic!(
    //     "{:?}",
    //     tree.map_data(&|_, s| (), &|_, x| x, &|_, x| x, Substack::Bottom)
    //   );
    // };
    Self { gen, suffix, tree }
  }
}

impl VirtFS for EmbeddedFS {
  fn get(&self, path: &[Tok<String>], full_path: PathSlice) -> FSResult {
    if path.is_empty() {
      return Ok(Loaded::collection(self.tree.keys(|_| true)));
    }
    let entry = (self.tree.walk1_ref(&[], path, |_| true))
      .map_err(|_| CodeNotFound::new(full_path.to_vpath()).pack())?;
    Ok(match &entry.0.member {
      ModMember::Item(text) => Loaded::Code(text.clone()),
      ModMember::Sub(sub) => Loaded::collection(sub.keys(|_| true)),
    })
  }
  fn display(&self, path: &[Tok<String>]) -> Option<String> {
    let Self { gen, suffix, .. } = self;
    Some(format!("{}{suffix} in {gen}", ev(path).join("/")))
  }
}
