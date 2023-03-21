use std::collections::HashMap;
use std::rc::Rc;
use itertools::Itertools;
use lasso::Spur;
use thiserror::Error;

use crate::utils::Stackframe;

use crate::ast::{Expr, Clause};

type ImportMap = HashMap<Spur, Rc<Vec<Spur>>>;

#[derive(Debug, Clone, Error)]
pub enum ResolutionError<Err> {
  #[error("Reference cycle at {0:?}")]
  Cycle(Vec<Rc<Vec<Spur>>>),
  #[error("No module provides {0:?}")]
  NoModule(Rc<Vec<Spur>>),
  #[error(transparent)]
  Delegate(#[from] Err)
}

type ResolutionResult<E> = Result<Rc<Vec<Spur>>, ResolutionError<E>>;

/// Recursively resolves symbols to their original names in expressions
/// while caching every resolution. This makes the resolution process
/// lightning fast and invalidation completely impossible since
/// the intermediate steps of a resolution aren't stored.
pub struct NameResolver<FSplit, FImps, E> {
  cache: HashMap<Rc<Vec<Spur>>, ResolutionResult<E>>,
  split: FSplit,
  get_imports: FImps
}

impl<FSplit, FImps, E> NameResolver<FSplit, FImps, E>
where
  FSplit: FnMut(Rc<Vec<Spur>>) -> Option<(Rc<Vec<Spur>>, Rc<Vec<Spur>>)>,
  FImps: FnMut(Rc<Vec<Spur>>) -> Result<ImportMap, E>,
  E: Clone
{
  pub fn new(split: FSplit, get_imports: FImps) -> Self {
    Self {
      cache: HashMap::new(),
      split,
      get_imports
    }
  }

  fn split(&self, symbol: Rc<Vec<Spur>>)
  -> Result<(Rc<Vec<Spur>>, Rc<Vec<Spur>>), ResolutionError<E>> {
    let (path, name) = (self.split)(symbol.clone())
      .ok_or_else(|| ResolutionError::NoModule(symbol.clone()))?;
    if name.is_empty() {
      panic!("get_modname matched all to module and nothing to name")
    }
    Ok((path, name))
  }

  /// Obtains a symbol's originnal name
  /// Uses a substack to detect loops
  fn find_origin_rec(
    &mut self,
    symbol: Rc<Vec<Spur>>,
    import_path: Stackframe<Rc<Vec<Spur>>>
  ) -> Result<Rc<Vec<Spur>>, ResolutionError<E>> {
    if let Some(cached) = self.cache.get(&symbol) {
      return cached.clone()
    }
    // The imports and path of the referenced file and the local name 
    let (path, name) = self.split(symbol)?;
    let imports = (self.get_imports)(path.clone())?;
    let result = if let Some(source) = imports.get(&name[0]) {
      let new_sym = source.iter().chain(name.iter()).cloned().collect_vec();
      if import_path.iter().any(|el| el.as_ref() == new_sym.as_slice()) {
        Err(ResolutionError::Cycle(import_path.iter().cloned().collect()))
      } else {
        self.find_origin_rec(Rc::new(new_sym), import_path.push(symbol.clone()))
      }
    } else {
      Ok(symbol.clone()) // If not imported, it must be locally defined
    };
    self.cache.insert(symbol, result.clone());
    result
  }

  fn process_exprv_rec(&mut self, exv: &[Expr]) -> Result<Vec<Expr>, ResolutionError<E>> {
    exv.iter().map(|ex| self.process_expression_rec(ex)).collect()
  }

  fn process_exprmrcopt_rec(&mut self,
    exbo: &Option<Rc<Expr>>
  ) -> Result<Option<Rc<Expr>>, ResolutionError<E>> {
    exbo.iter().map(|exb| Ok(Rc::new(self.process_expression_rec(exb)?)))
      .next().transpose()
  }

  fn process_clause_rec(&mut self, tok: &Clause) -> Result<Clause, ResolutionError<E>> {
    Ok(match tok {
      Clause::S(c, exv) => Clause::S(*c, Rc::new(
        exv.iter().map(|e| self.process_expression_rec(e))
          .collect::<Result<_, _>>()?
      )),
      Clause::Lambda(name, typ, body) =>  Clause::Lambda(name.clone(),
        Rc::new(self.process_exprv_rec(&typ)?),
        Rc::new(self.process_exprv_rec(&body)?)
      ),
      Clause::Auto(name, typ, body) => Clause::Auto(name.clone(),
        Rc::new(self.process_exprv_rec(&typ)?),
        Rc::new(self.process_exprv_rec(&body)?)
      ),
      Clause::Name(name) => Clause::Name(self.find_origin(name.clone())?),
      x => x.clone()
    })
  }

  fn process_expression_rec(&mut self, Expr(token, typ): &Expr) -> Result<Expr, ResolutionError<E>> {
    Ok(Expr(
      self.process_clause_rec(token)?,
      Rc::new(typ.iter().map(|t| {
        self.process_clause_rec(t)
      }).collect::<Result<_, _>>()?)
    ))
  }

  pub fn find_origin(&mut self, symbol: Rc<Vec<Spur>>) -> Result<Rc<Vec<Spur>>, ResolutionError<E>> {
    self.find_origin_rec(symbol.clone(), Stackframe::new(symbol))
  }

  #[allow(dead_code)]
  pub fn process_clause(&mut self, clause: &Clause) -> Result<Clause, ResolutionError<E>> {
    self.process_clause_rec(clause)
  }

  pub fn process_expression(&mut self, ex: &Expr) -> Result<Expr, ResolutionError<E>> {
    self.process_expression_rec(ex)
  }
}
