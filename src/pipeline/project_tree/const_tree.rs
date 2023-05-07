use std::{ops::Add, rc::Rc};

use hashbrown::HashMap;

use crate::representations::tree::{ModEntry, ModMember, Module};
use crate::representations::Primitive;
use crate::representations::location::Location;
use crate::foreign::ExternFn;
use crate::interner::{Token, Interner};
use crate::ast::{Expr, Clause};
use crate::utils::{Substack, pushed};

use super::{ProjectModule, ProjectExt, ProjectTree};

pub enum ConstTree {
  Const(Expr),
  Tree(HashMap<Token<String>, ConstTree>)
}
impl ConstTree {
  pub fn xfn(xfn: impl ExternFn + 'static) -> Self {
    Self::Const(Expr{
      location: Location::Unknown,
      value: Clause::P(Primitive::ExternFn(Box::new(xfn)))
    })
  }
  pub fn tree(
    arr: impl IntoIterator<Item = (Token<String>, Self)>
  ) -> Self {
    Self::Tree(arr.into_iter().collect())
  }
}
impl Add for ConstTree {
  type Output = ConstTree;

  fn add(self, rhs: ConstTree) -> Self::Output {
    if let (Self::Tree(t1), Self::Tree(mut t2)) = (self, rhs) {
      let mut product = HashMap::new();
      for (key, i1) in t1 {
        if let Some(i2) = t2.remove(&key) {
          product.insert(key, i1 + i2);
        } else {
          product.insert(key, i1);
        }
      }
      product.extend(t2.into_iter());
      Self::Tree(product)
    } else {
      panic!("cannot combine tree and value fields")
    }
  }
}

fn from_const_tree_rec(
  path: Substack<Token<String>>,
  consts: HashMap<Token<String>, ConstTree>,
  file: &[Token<String>],
  i: &Interner,
) -> ProjectModule {
  let mut items = HashMap::new();
  let path_v = path.iter().rev_vec_clone();
  for (name, item) in consts {
    items.insert(name, ModEntry{
      exported: true,
      member: match item {
        ConstTree::Const(c) => ModMember::Item(c),
        ConstTree::Tree(t) => ModMember::Sub(Rc::new(
          from_const_tree_rec(path.push(name), t, file, i)
        )),
      }
    });
  }
  let exports = items.keys()
    .map(|name| (*name, i.i(&pushed(&path_v, *name))))
    .collect();
  Module {
    items,
    imports: vec![],
    extra: ProjectExt {
      exports,
      file: Some(file.to_vec()),
      ..Default::default()
    }
  }
}

pub fn from_const_tree(
  consts: HashMap<Token<String>, ConstTree>,
  file: &[Token<String>],
  i: &Interner,
) -> ProjectTree {
  let module = from_const_tree_rec(Substack::Bottom, consts, file, i);
  ProjectTree(Rc::new(module))
}