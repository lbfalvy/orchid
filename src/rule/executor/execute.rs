use hashbrown::HashMap;
use mappable_rc::Mrc;

use crate::unwrap_or;
use crate::utils::{to_mrc_slice, one_mrc_slice, mrc_empty_slice};
use crate::utils::iter::{box_once, into_boxed_iter};
use crate::ast::{Expr, Clause};
use super::slice_matcher::SliceMatcherDnC;
use super::state::{State, Entry};
use super::super::RuleError;

fn verify_scalar_vec(pattern: &Expr, is_vec: &mut HashMap<String, bool>)
-> Result<(), String> {
  let verify_clause = |clause: &Clause, is_vec: &mut HashMap<String, bool>| -> Result<(), String> {
    match clause {
      Clause::Placeh{key, vec} => {
        if let Some(known) = is_vec.get(key) {
          if known != &vec.is_some() { return Err(key.to_string()) }
        } else {
          is_vec.insert(key.clone(), vec.is_some());
        }
      }
      Clause::Auto(name, typ, body) => {
        if let Some(key) = name.as_ref().and_then(|key| key.strip_prefix('$')) {
          if is_vec.get(key) == Some(&true) { return Err(key.to_string()) }
        }
        typ.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
        body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
      }
      Clause::Lambda(name, typ, body) => {
        if let Some(key) = name.strip_prefix('$') {
          if is_vec.get(key) == Some(&true) { return Err(key.to_string()) }
        }
        typ.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
        body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
      }
      Clause::S(_, body) => {
        body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
      }
      _ => ()
    };
    Ok(())
  };
  let Expr(val, typ) = pattern;
  verify_clause(val, is_vec)?;
  for typ in typ.as_ref() {
    verify_clause(typ, is_vec)?;
  }
  Ok(())
}


fn slice_to_vec(src: &mut Mrc<[Expr]>, tgt: &mut Mrc<[Expr]>) {
  let prefix_expr = Expr(Clause::Placeh{key: "::prefix".to_string(), vec: Some((0, false))}, to_mrc_slice(vec![]));
  let postfix_expr = Expr(Clause::Placeh{key: "::postfix".to_string(), vec: Some((0, false))}, to_mrc_slice(vec![]));
  // Prefix or postfix to match the full vector
  let head_multi = matches!(src.first().expect("Src can never be empty!").0, Clause::Placeh{vec: Some(_), ..});
  let tail_multi = matches!(src.last().expect("Impossible branch!").0, Clause::Placeh{vec: Some(_), ..});
  let prefix_vec = if head_multi {vec![]} else {vec![prefix_expr]};
  let postfix_vec = if tail_multi {vec![]} else {vec![postfix_expr]};
  *src = to_mrc_slice(prefix_vec.iter().chain(src.iter()).chain(postfix_vec.iter()).cloned().collect());
  *tgt = to_mrc_slice(prefix_vec.iter().chain(tgt.iter()).chain(postfix_vec.iter()).cloned().collect());
}

/// Traverse the tree, calling pred on every sibling list until it returns some vec
/// then replace the sibling list with that vec and return true
/// return false if pred never returned some
fn update_first_seq_rec<F>(input: Mrc<[Expr]>, pred: &mut F) -> Option<Mrc<[Expr]>>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  if let o@Some(_) = pred(Mrc::clone(&input)) {o} else {
    for Expr(cls, _) in input.iter() {
      if let Some(t) = cls.typ() {
        if let o@Some(_) = update_first_seq_rec(t, pred) {return o}
      }
      if let Some(b) = cls.body() {
        if let o@Some(_) = update_first_seq_rec(b, pred) {return o}
      }
    }
    None
  }
}

/// keep re-probing the input with pred until it stops matching
fn update_all_seqs<F>(input: Mrc<[Expr]>, pred: &mut F) -> Option<Mrc<[Expr]>>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  let mut tmp = update_first_seq_rec(input, pred);
  while let Some(xv) = tmp {
    tmp = update_first_seq_rec(Mrc::clone(&xv), pred);
    if tmp.is_none() {return Some(xv)}
  }
  None
}

// fn write_clause_rec(state: &State, clause: &Clause) -> 

fn write_expr_rec(state: &State, Expr(tpl_clause, tpl_typ): &Expr) -> Box<dyn Iterator<Item = Expr>> {
  let out_typ = tpl_typ.iter()
    .flat_map(|c| write_expr_rec(state, &c.clone().into_expr()))
    .map(Expr::into_clause)
    .collect::<Mrc<[Clause]>>();
  match tpl_clause {
    Clause::Auto(name_opt, typ, body) => box_once(Expr(Clause::Auto(
      name_opt.as_ref().and_then(|name| {
        if let Some(state_key) = name.strip_prefix('$') {
          match &state[state_key] {
            Entry::NameOpt(name) => name.as_ref().map(|s| s.as_ref().to_owned()),
            Entry::Name(name) => Some(name.as_ref().to_owned()),
            _ => panic!("Auto template name may only be derived from Auto or Lambda name")
          }
        } else {
          Some(name.to_owned())
        }
      }),
      write_slice_rec(state, typ),
      write_slice_rec(state, body)
    ), out_typ.to_owned())),
    Clause::Lambda(name, typ, body) => box_once(Expr(Clause::Lambda(
      if let Some(state_key) = name.strip_prefix('$') {
        if let Entry::Name(name) = &state[state_key] {
          name.as_ref().to_owned()
        } else {panic!("Lambda template name may only be derived from Lambda name")}
      } else {
        name.to_owned()
      },
      write_slice_rec(state, typ),
      write_slice_rec(state, body)
    ), out_typ.to_owned())),
    Clause::S(c, body) => box_once(Expr(Clause::S(
      *c,
      write_slice_rec(state, body)
    ), out_typ.to_owned())),
    Clause::Placeh{key, vec: None} => {
      let real_key = unwrap_or!(key.strip_prefix('_'); key);
      match &state[real_key] {
        Entry::Scalar(x) => box_once(x.as_ref().to_owned()),
        Entry::Name(n) => box_once(Expr(Clause::Name {
          local: Some(n.as_ref().to_owned()),
          qualified: one_mrc_slice(n.as_ref().to_owned())
        }, mrc_empty_slice())),
        _ => panic!("Scalar template may only be derived from scalar placeholder"),
      }
    },
    Clause::Placeh{key, vec: Some(_)} => if let Entry::Vec(v) = &state[key] {
      into_boxed_iter(v.as_ref().to_owned())
    } else {panic!("Vectorial template may only be derived from vectorial placeholder")},
    Clause::Explicit(param) => {
      assert!(out_typ.len() == 0, "Explicit should never have a type annotation");
      box_once(Clause::Explicit(Mrc::new(
        Clause::from_exprv(write_expr_rec(state, param).collect())
          .expect("Result shorter than template").into_expr()
      )).into_expr())
    },
    // Explicit base case so that we get an error if Clause gets new values
    c@Clause::Literal(_) | c@Clause::Name { .. } | c@Clause::ExternFn(_) | c@Clause::Atom(_) =>
      box_once(Expr(c.to_owned(), out_typ.to_owned()))
  }
}

/// Fill in a template from a state as produced by a pattern
fn write_slice_rec(state: &State, tpl: &Mrc<[Expr]>) -> Mrc<[Expr]> {
  eprintln!("Writing {tpl:?} with state {state:?}");
  tpl.iter().flat_map(|xpr| write_expr_rec(state, xpr)).collect()
}

/// Apply a rule (a pair of pattern and template) to an expression
pub fn execute(mut src: Mrc<[Expr]>, mut tgt: Mrc<[Expr]>, input: Mrc<[Expr]>)
-> Result<Option<Mrc<[Expr]>>, RuleError> {
  // Dimension check
  let mut is_vec_db = HashMap::new();
  src.iter().try_for_each(|e| verify_scalar_vec(e, &mut is_vec_db))
    .map_err(RuleError::ScalarVecMismatch)?;
  tgt.iter().try_for_each(|e| verify_scalar_vec(e, &mut is_vec_db))
    .map_err(RuleError::ScalarVecMismatch)?;
  // Padding
  slice_to_vec(&mut src, &mut tgt);
  // Generate matcher
  let matcher = SliceMatcherDnC::new(src);
  let matcher_cache = SliceMatcherDnC::get_matcher_cache();
  Ok(update_all_seqs(Mrc::clone(&input), &mut |p| {
    let state = matcher.match_range_cached(p, &matcher_cache)?;
    Some(write_slice_rec(&state, &tgt))
  }))
}
