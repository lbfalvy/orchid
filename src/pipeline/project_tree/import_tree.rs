use std::cmp::Ordering;
use std::fmt::Display;
use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::error::{ProjectError, ProjectResult};
use crate::pipeline::source_loader::{PreMod, Preparsed};
use crate::representations::project::ImpReport;
use crate::sourcefile::{absolute_path, Import};
use crate::tree::{ErrKind, ModEntry, ModMember, Module, WalkError};
use crate::utils::boxed_iter::{box_chain, box_once};
use crate::utils::pure_push::pushed_ref;
use crate::utils::{unwrap_or, BoxedIter};
use crate::{Interner, ProjectTree, Tok, VName};

pub type ImpMod = Module<ImpReport<VName>, ()>;

/// Assert that a module identified by a path can see a given symbol
fn assert_visible<'a>(
  source: &'a [Tok<String>], // must point to a file or submodule
  target: &'a [Tok<String>],
  root: &'a Module<impl Clone + Display, impl Clone + Display>,
) -> Result<(), WalkError<'a>> {
  if target.split_last().map_or(true, |(_, m)| source.starts_with(m)) {
    // The global module (empty path) is always visible
    return Ok(()); // Symbols in ancestor modules are always visible
  }
  // walk the first section where visibility is ignored
  let shared_len =
    source.iter().zip(target.iter()).take_while(|(a, b)| a == b).count();
  let (shared_path, deep_path) = target.split_at(shared_len + 1);
  let private_root = root.walk_ref(&[], shared_path, false)?;
  // walk the second part where visibility matters
  private_root.walk1_ref(shared_path, deep_path, true)?;
  Ok(())
}

pub fn assert_visible_overlay<'a>(
  source: &'a [Tok<String>], // must point to a file or submodule
  target: &'a [Tok<String>],
  first: &'a Module<impl Clone + Display, impl Clone + Display>,
  fallback: &'a Module<impl Clone + Display, impl Clone + Display>,
) -> Result<(), WalkError<'a>> {
  assert_visible(source, target, first).or_else(|e1| {
    if e1.kind == ErrKind::Missing {
      match assert_visible(source, target, fallback) {
        // if both are walk errors, report the longer of the two
        Err(mut e2) if e2.kind == ErrKind::Missing =>
          Err(match e1.depth().cmp(&e2.depth()) {
            Ordering::Less => e2,
            Ordering::Greater => e1,
            Ordering::Equal => {
              e2.options = box_chain!(e2.options, e1.options);
              e2
            },
          }),
        // otherwise return the parent's result
        x => x,
      }
    } else {
      Err(e1)
    }
  })
}

pub fn process_donor_module<'a, TItem: Clone>(
  module: &'a Module<TItem, impl Clone>,
  abs_path: Rc<VName>,
  is_op: impl Fn(&TItem) -> bool + 'a,
) -> impl Iterator<Item = (Tok<String>, VName, bool)> + 'a {
  (module.entries.iter()).filter(|(_, ent)| ent.exported).map(
    move |(n, ent)| {
      let is_op = ent.item().map_or(false, &is_op);
      (n.clone(), pushed_ref(abs_path.as_ref(), n.clone()), is_op)
    },
  )
}

pub fn import_tree(
  modpath: VName,
  pre: &PreMod,
  root: &Preparsed,
  prev_root: &ProjectTree<VName>,
  i: &Interner,
) -> ProjectResult<ImpMod> {
  let imports = pre.extra.details().map_or(&[][..], |e| &e.imports[..]);
  let entries = (imports.iter())
    // imports become leaf sets
    .map(|Import { name, path, location }| -> ProjectResult<BoxedIter<_>> {
      let mut abs_path = absolute_path(&modpath, path, i, location)?;
      Ok(if let Some(name) = name {
        // named imports are validated and translated 1->1
        abs_path.push(name.clone());
        assert_visible_overlay(&modpath, &abs_path, &root.0, &prev_root.0)
          .map_err(|e| -> Rc<dyn ProjectError> {
            println!("Current root: {}", &root.0);
            // println!("Old root: {:#?}", &prev_root.0);
            panic!("{}", e.at(location))
          })?;
        let is_op = (root.0.walk1_ref(&[], &abs_path, false))
          .map(|(ent, _)| ent.item().map_or(false, |i| i.is_op))
          .or_else(|e| if e.kind == ErrKind::Missing {
            (prev_root.0.walk1_ref(&[], &abs_path, false))
              .map(|(ent, _)| ent.item().map_or(false, |i| i.is_op))
          } else {Err(e)})
          .map_err(|e| e.at(location))?;
        box_once((name.clone(), abs_path, is_op))
      } else {
        let rc_path = Rc::new(abs_path);
        // wildcard imports are validated
        assert_visible_overlay(&modpath, &rc_path, &root.0, &prev_root.0)
          .map_err(|e| e.at(location))?;
        // and instantiated for all exports of the target 1->n
        let new_imports = match (root.0).walk_ref(&[], &rc_path, false) {
          Err(e) if e.kind == ErrKind::Missing => Err(e),
          Err(e) => return Err(e.at(location)),
          Ok(module)
            => Ok(process_donor_module(module, rc_path.clone(), |i| i.is_op))
        };
        let old_m = match (prev_root.0).walk_ref(&[], &rc_path, false) {
          Err(e) if e.kind != ErrKind::Missing => return Err(e.at(location)),
          Err(e1) => match new_imports {
            Ok(it) => return Ok(Box::new(it)),
            Err(mut e2) => return Err(match e1.depth().cmp(&e2.depth()) {
              Ordering::Less => e2.at(location),
              Ordering::Greater => e1.at(location),
              Ordering::Equal => {
                e2.options = box_chain!(e2.options, e1.options);
                e2.at(location)
              },
            }),
          },
          Ok(old_m) => old_m,
        };
        let it1 = process_donor_module(old_m, rc_path.clone(), |i| i.is_op);
        match new_imports {
          Err(_) => Box::new(it1),
          Ok(it2) => box_chain!(it1, it2)
        }
      })
    })
    // leaf sets flattened to leaves
    .flatten_ok()
    // translated to entries
    .map_ok(|(name, source, is_op)| {
      (name, ModEntry {
        exported: false, // this is irrelevant but needed
        member: ModMember::Item(ImpReport { source, is_op }),
      })
    })
    .chain(
      (pre.entries.iter())
        // recurse on submodules
        .filter_map(|(k, v)| {
          Some((k, v, unwrap_or!(&v.member => ModMember::Sub; return None)))
        })
        .map(|(k, v, pre)| {
          let path = pushed_ref(&modpath, k.clone());
          Ok((k.clone(), ModEntry {
            exported: v.exported,
            member: ModMember::Sub(import_tree(path, pre, root, prev_root, i)?),
          }))
        }),
    )
    .collect::<Result<HashMap<_, _>, _>>()?;
  Ok(Module { extra: (), entries })
}
