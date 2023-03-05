use std::fmt::{Display, Debug};
use std::rc::Rc;

use crate::utils::Side;
use crate::foreign::{ExternError, Atom};

use super::path_set::PathSet;
use super::primitive::Primitive;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Clause {
  P(Primitive),
  Apply{
    f: Rc<Self>,
    x: Rc<Self>,
    id: usize
  },
  Lambda{
    args: Option<PathSet>,
    body: Rc<Self>
  },
  LambdaArg,
}

impl Debug for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Clause::P(p) => write!(f, "{p:?}"),
      Clause::LambdaArg => write!(f, "arg"),
      Clause::Apply { f: fun, x, id } => write!(f, "({:?} {:?})@{}", fun.as_ref(), x.as_ref(), id),
      Clause::Lambda { args, body } => {
        write!(f, "\\")?;
        match args {
          Some(path) => write!(f, "{path:?}")?,
          None => write!(f, "_")?,
        }
        write!(f, ".")?;
        write!(f, "{:?}", body.as_ref())
      }
    }
  }
}

/// Problems in the process of execution
#[derive(Clone)]
pub enum RuntimeError {
  Extern(Rc<dyn ExternError>),
  NonFunctionApplication(usize),
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Extern(e) => write!(f, "Error in external function: {e}"),
      Self::NonFunctionApplication(loc) => write!(f, "Primitive applied as function at {loc}")
    }
  }
}

/// Various reasons why a new clause might not have been produced
#[derive(Clone)]
pub enum InternalError {
  Runtime(RuntimeError),
  NonReducible
}

fn map_at<E, F: FnOnce(&Clause) -> Result<Clause, E>>(
  path: &[Side], source: &Clause, mapper: F
) -> Result<Clause, E> {
  // Pass right through lambdas
  if let Clause::Lambda { args, body } = source {
    return Ok(Clause::Lambda {
      args: args.clone(),
      body: Rc::new(map_at(path, body, mapper)?)
    })
  }
  // If the path ends here, process the next (non-lambda) node
  let (head, tail) = if let Some(sf) = path.split_first() {sf} else {
    return mapper(source)
  };
  // If it's an Apply, execute the next step in the path
  if let Clause::Apply { f, x, id } = source {
    return Ok(match head {
      Side::Left => Clause::Apply {
        f: Rc::new(map_at(tail, f, mapper)?),
        x: x.clone(),
        id: *id
      },
      Side::Right => Clause::Apply {
        f: f.clone(),
        x: Rc::new(map_at(tail, x, mapper)?),
        id: *id
      }
    })
  }
  panic!("Invalid path")
}

fn substitute(PathSet { steps, next }: &PathSet, value: &Clause, body: &Clause) -> Clause {
  map_at(&steps, body, |checkpoint| -> Result<Clause, !> {
    match (checkpoint, next) {
      (Clause::Lambda{..}, _) =>  unreachable!("Handled by map_at"),
      (Clause::Apply { f, x, id }, Some((left, right))) => Ok(Clause::Apply {
        f: Rc::new(substitute(left, value, f)),
        x: Rc::new(substitute(right, value, x)),
        id: *id
      }),
      (Clause::LambdaArg, None) => Ok(value.clone()),
      (_, None) => panic!("Substitution path ends in something other than LambdaArg"),
      (_, Some(_)) => panic!("Substitution path leads into something other than Apply"),
    }
  }).into_ok()
}

fn apply(f: &Clause, x: Rc<Clause>, id: usize) -> Result<Clause, InternalError> {
  match f {
    Clause::P(Primitive::ExternFn(f)) => f.apply(x.as_ref().clone())
      .map_err(|e| InternalError::Runtime(RuntimeError::Extern(e))),
    fex@Clause::Apply{..} => Ok(Clause::Apply{ // Don't execute the pre-function expression
      f: Rc::new(fex.run_once()?), // take a step in resolving it instead
      x, id
    }),
    Clause::Lambda{args, body} => Ok(if let Some(args) = args {
      substitute(args, x.as_ref(), body)
    } else {body.as_ref().clone()}),
    _ => Err(InternalError::Runtime(RuntimeError::NonFunctionApplication(id)))
  }
}

impl Clause {
  pub fn run_once(&self) -> Result<Self, InternalError> {
    match self {
      Clause::Apply{f, x, id} => apply(f.as_ref(), x.clone(), *id),
      Clause::P(Primitive::Atom(Atom(data))) => data.run_once(),
      _ => Err(InternalError::NonReducible)
    }
  }

  pub fn run_n_times(&self, n: usize) -> Result<(Self, usize), RuntimeError> {
    let mut i = self.clone();
    let mut done = 0;
    while done < n {
      match match &i {
        Clause::Apply{f, x, id} => match apply(f.as_ref(), x.clone(), *id) {
          Err(e) => Err(e),
          Ok(c) => {
            i = c;
            done += 1;
            Ok(())
          }
        },
        Clause::P(Primitive::Atom(Atom(data))) => match data.run_n_times(n - done) {
          Err(e) => Err(InternalError::Runtime(e)),
          Ok((c, n)) => {
            i = c;
            done += n;
            Ok(())
          }
        },
        _ => Err(InternalError::NonReducible)
      } {
        Err(InternalError::NonReducible) => return Ok((i, done)),
        Err(InternalError::Runtime(e)) => return Err(e),
        Ok(()) => ()
      }
    }
    return Ok((i, done));
  }

  pub fn run_to_completion(&self) -> Result<Self, RuntimeError> {
    let mut i = self.clone();
    loop {
      match match &i {
        Clause::Apply { f, x, id } => match apply(f.as_ref(), x.clone(), *id) {
          Err(e) => Err(e),
          Ok(c) => Ok(i = c)
        },
        Clause::P(Primitive::Atom(Atom(data))) => match data.run_to_completion() {
          Err(e) => Err(InternalError::Runtime(e)),
          Ok(c) => Ok(i = c)
        },
        _ => Err(InternalError::NonReducible)
      } {
        Err(InternalError::NonReducible) => break,
        Err(InternalError::Runtime(e)) => return Err(e),
        Ok(()) => ()
      }
    };
    Ok(i)
  }
}