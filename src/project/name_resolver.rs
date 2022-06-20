use std::{collections::HashMap};
use thiserror::Error;

use crate::utils::Substack;

use super::expr::{Expr, Token};

type ImportMap = HashMap<String, Vec<String>>;

#[derive(Debug, Clone, Error)]
pub enum ResolutionError<Err> {
    #[error("Reference cycle at {0:?}")]
    Cycle(Vec<Vec<String>>),
    #[error("No module provides {0:?}")]
    NoModule(Vec<String>),
    #[error(transparent)]
    Delegate(#[from] Err)
}

/// Recursively resolves symbols to their original names in expressions while caching every
/// resolution. This makes the resolution process lightning fast and invalidation completely
/// impossible since the intermediate steps of a resolution aren't stored.
pub struct NameResolver<FSplit, FImps, E> {
    cache: HashMap<Vec<String>, Result<Vec<String>, ResolutionError<E>>>,
    get_modname: FSplit,
    get_imports: FImps
}


impl<FSplit, FImps, E> NameResolver<FSplit, FImps, E>
where
    FSplit: FnMut(&Vec<String>) -> Option<Vec<String>>,
    FImps: FnMut(&Vec<String>) -> Result<ImportMap, E>,
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
        symbol: &Vec<String>,
        import_path: &Substack<'_, &Vec<String>>
    ) -> Result<Vec<String>, ResolutionError<E>> {
        if let Some(cached) = self.cache.get(symbol) { return cached.clone() }
        // The imports and path of the referenced file and the local name 
        let mut splitpoint = symbol.len();
        let path = (self.get_modname)(symbol).ok_or(ResolutionError::NoModule(symbol.clone()))?;
        let name = symbol.split_at(path.len()).1;
        let imports = (self.get_imports)(&path)?;
        let result = if let Some(source) = imports.get(&name[0]) {
            let new_sym: Vec<String> = source.iter().chain(name.iter()).cloned().collect();
            if import_path.iter().any(|el| el == &&new_sym) {
                Err(ResolutionError::Cycle(import_path.iter().cloned().cloned().collect()))
            } else {
                self.find_origin_rec(&new_sym, &import_path.push(symbol))
            }
        } else {
            Ok(symbol.clone()) // If not imported, it must be locally defined
        };
        self.cache.insert(symbol.clone(), result.clone());
        return result
    }

    fn process_exprv_rec(&mut self, exv: &[Expr]) -> Result<Vec<Expr>, ResolutionError<E>> {
        exv.iter().map(|ex| self.process_expression_rec(ex)).collect()
    }

    fn process_exprboxopt_rec(&mut self,
        exbo: &Option<Box<Expr>>
    ) -> Result<Option<Box<Expr>>, ResolutionError<E>> {
        exbo.iter().map(|exb| Ok(Box::new(self.process_expression_rec(exb.as_ref())?)))
            .next().transpose()
    }

    fn process_token_rec(&mut self, tok: &Token) -> Result<Token, ResolutionError<E>> {
        Ok(match tok {
            Token::Literal(l) => Token::Literal(l.clone()),
            Token::S(exv) => Token::S(
                exv.iter().map(|e| self.process_expression_rec(e))
                    .collect::<Result<Vec<Expr>, ResolutionError<E>>>()?
            ),
            Token::Lambda(name, typ, body) =>  Token::Lambda(name.clone(),
                self.process_exprboxopt_rec(typ)?,
                self.process_exprv_rec(body)?
            ),
            Token::Auto(name, typ, body) => Token::Auto(name.clone(),
                self.process_exprboxopt_rec(typ)?,
                self.process_exprv_rec(body)?
            ),
            Token::Name { qualified, local } => Token::Name {
                local: local.clone(),
                qualified: self.find_origin(qualified)?
            }
        })
    }

    fn process_expression_rec(&mut self, ex: &Expr) -> Result<Expr, ResolutionError<E>> {
        Ok(Expr {
            token: self.process_token_rec(&ex.token)?,
            typ: self.process_exprboxopt_rec(&ex.typ)?
        })
    }

    pub fn find_origin(&mut self, symbol: &Vec<String>) -> Result<Vec<String>, ResolutionError<E>> {
        self.find_origin_rec(symbol, &Substack::new(symbol))
    }

    pub fn process_token(&mut self, tok: &Token) -> Result<Token, ResolutionError<E>> {
        self.process_token_rec(tok)
    }

    pub fn process_expression(&mut self, ex: &Expr) -> Result<Expr, ResolutionError<E>> {
        self.process_expression_rec(ex)
    }
}