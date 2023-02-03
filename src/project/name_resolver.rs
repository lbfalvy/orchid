use std::collections::HashMap;
use mappable_rc::Mrc;
use thiserror::Error;

use crate::utils::{Stackframe, to_mrc_slice};

use crate::ast::{Expr, Clause};

type ImportMap = HashMap<String, Mrc<[String]>>;

#[derive(Debug, Clone, Error)]
pub enum ResolutionError<Err> {
  #[error("Reference cycle at {0:?}")]
  Cycle(Vec<Mrc<[String]>>),
  #[error("No module provides {0:?}")]
  NoModule(Mrc<[String]>),
  #[error(transparent)]
  Delegate(#[from] Err)
}

type ResolutionResult<E> = Result<Mrc<[String]>, ResolutionError<E>>;

/// Recursively resolves symbols to their original names in expressions while caching every
/// resolution. This makes the resolution process lightning fast and invalidation completely
/// impossible since the intermediate steps of a resolution aren't stored.
pub struct NameResolver<FSplit, FImps, E> {
  cache: HashMap<Mrc<[String]>, ResolutionResult<E>>,
  get_modname: FSplit,
  get_imports: FImps
}

impl<FSplit, FImps, E> NameResolver<FSplit, FImps, E>
where
  FSplit: FnMut(Mrc<[String]>) -> Option<Mrc<[String]>>,
  FImps: FnMut(Mrc<[String]>) -> Result<ImportMap, E>,
  E: Clone
{
  pub fn new(get_modname: FSplit, get_imports: FImps) -> Self {
    Self {
      cache: HashMap::new(),
      get_modname,
      get_imports
    }
  }

  /// Obtains a symbol's originnal name
  /// Uses a substack to detect loops
  fn find_origin_rec(
    &mut self,
    symbol: Mrc<[String]>,
    import_path: Stackframe<Mrc<[String]>>
  ) -> Result<Mrc<[String]>, ResolutionError<E>> {
    if let Some(cached) = self.cache.get(&symbol) {
      return cached.as_ref().map_err(|e| e.clone()).map(Mrc::clone)
    }
    // The imports and path of the referenced file and the local name 
    let path = (self.get_modname)(Mrc::clone(&symbol)).ok_or_else(|| {
      ResolutionError::NoModule(Mrc::clone(&symbol))
    })?;
    let name = &symbol[path.len()..];
    if name.is_empty() {
      panic!("get_modname matched all to module and nothing to name in {:?}", import_path)
    }
    let imports = (self.get_imports)(Mrc::clone(&path))?;
    let result = if let Some(source) = imports.get(&name[0]) {
      let new_sym: Vec<String> = source.iter().chain(name.iter()).cloned().collect();
      if import_path.iter().any(|el| el.as_ref() == new_sym.as_slice()) {
        Err(ResolutionError::Cycle(import_path.iter().map(Mrc::clone).collect()))
      } else {
        self.find_origin_rec(to_mrc_slice(new_sym), import_path.push(Mrc::clone(&symbol)))
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
    exbo: &Option<Mrc<Expr>>
  ) -> Result<Option<Mrc<Expr>>, ResolutionError<E>> {
    exbo.iter().map(|exb| Ok(Mrc::new(self.process_expression_rec(exb.as_ref())?)))
      .next().transpose()
  }

  fn process_clause_rec(&mut self, tok: &Clause) -> Result<Clause, ResolutionError<E>> {
    Ok(match tok {
      Clause::S(c, exv) => Clause::S(*c, to_mrc_slice(
        exv.as_ref().iter().map(|e| self.process_expression_rec(e))
          .collect::<Result<Vec<Expr>, ResolutionError<E>>>()?
      )),
      Clause::Lambda(name, typ, body) =>  Clause::Lambda(name.clone(),
        to_mrc_slice(self.process_exprv_rec(typ.as_ref())?),
        to_mrc_slice(self.process_exprv_rec(body.as_ref())?)
      ),
      Clause::Auto(name, typ, body) => Clause::Auto(name.clone(),
        to_mrc_slice(self.process_exprv_rec(typ.as_ref())?),
        to_mrc_slice(self.process_exprv_rec(body.as_ref())?)
      ),
      Clause::Name{local, qualified} => Clause::Name{
        local: local.clone(),
        qualified:  self.find_origin(Mrc::clone(qualified))?
      },
      x => x.clone()
    })
  }

  fn process_expression_rec(&mut self, Expr(token, typ): &Expr) -> Result<Expr, ResolutionError<E>> {
    Ok(Expr(
      self.process_clause_rec(token)?,
      typ.iter().map(|t| self.process_clause_rec(t)).collect::<Result<_, _>>()?
    ))
  }

  pub fn find_origin(&mut self, symbol: Mrc<[String]>) -> Result<Mrc<[String]>, ResolutionError<E>> {
    self.find_origin_rec(Mrc::clone(&symbol), Stackframe::new(symbol))
  }

  #[allow(dead_code)]
  pub fn process_clause(&mut self, clause: &Clause) -> Result<Clause, ResolutionError<E>> {
    self.process_clause_rec(clause)
  }

  pub fn process_expression(&mut self, ex: &Expr) -> Result<Expr, ResolutionError<E>> {
    self.process_expression_rec(ex)
  }
}
