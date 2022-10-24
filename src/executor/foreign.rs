use std::any::Any;
use std::fmt::{Display, Debug};
use std::hash::Hash;

use mappable_rc::Mrc;

use crate::representations::typed::{Expr, Clause};

pub trait ExternError: Display {}

/// Represents an externally defined function from the perspective of the executor
/// Since Orchid lacks basic numerical operations, these are also external functions.
#[derive(Eq)]
pub struct ExternFn {
    name: String, param: Mrc<Expr>, rttype: Mrc<Expr>,
    function: Mrc<dyn Fn(Clause) -> Result<Clause, Mrc<dyn ExternError>>>
}

impl ExternFn {
    pub fn new<F: 'static + Fn(Clause) -> Result<Clause, Mrc<dyn ExternError>>>(
        name: String, param: Mrc<Expr>, rttype: Mrc<Expr>, f: F
    ) -> Self {
        Self {
            name, param, rttype,
            function: Mrc::map(Mrc::new(f), |f| {
                f as &dyn Fn(Clause) -> Result<Clause, Mrc<dyn ExternError>>
            })
        }
    }
    fn name(&self) -> &str {&self.name}
    fn apply(&self, arg: Clause) -> Result<Clause, Mrc<dyn ExternError>> {(self.function)(arg)}
}

impl Clone for ExternFn { fn clone(&self) -> Self { Self {
    name: self.name.clone(),
    param: Mrc::clone(&self.param),
    rttype: Mrc::clone(&self.rttype),
    function: Mrc::clone(&self.function)
}}}
impl PartialEq for ExternFn { fn eq(&self, other: &Self) -> bool { self.name() == other.name() }}
impl Hash for ExternFn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.name.hash(state) }
}
impl Debug for ExternFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "##EXTERN[{}]:{:?} -> {:?}##", self.name(), self.param, self.rttype)
    }
}

pub trait Atomic: Any + Debug where Self: 'static {
    fn as_any(&self) -> &dyn Any;
    fn definitely_eq(&self, _other: &dyn Any) -> bool;
    fn hash(&self, hasher: &mut dyn std::hash::Hasher);
}

/// Represents a unit of information from the perspective of the executor. This may be
/// something like a file descriptor which functions can operate on, but it can also be
/// information in the universe of types or kinds such as the type of signed integers or
/// the kind of types. Ad absurdum it can also be just a number, although Literal is
/// preferable for types it's defined on.
#[derive(Eq)]
pub struct Atom {
    typ: Mrc<Expr>,
    data: Mrc<dyn Atomic>
}
impl Atom {
    pub fn new<T: 'static + Atomic>(data: T, typ: Mrc<Expr>) -> Self { Self{
        typ,
        data: Mrc::map(Mrc::new(data), |d| d as &dyn Atomic)
    } }
    pub fn data(&self) -> &dyn Atomic { self.data.as_ref() as &dyn Atomic }
    pub fn try_cast<T: Atomic>(&self) -> Result<&T, ()> {
        self.data().as_any().downcast_ref().ok_or(())
    }
    pub fn is<T: 'static>(&self) -> bool { self.data().as_any().is::<T>() }
    pub fn cast<T: 'static>(&self) -> &T {
        self.data().as_any().downcast_ref().expect("Type mismatch on Atom::cast")
    }
}

impl Clone for Atom {
    fn clone(&self) -> Self { Self {
        typ: Mrc::clone(&self.typ),
        data: Mrc::clone(&self.data)
    } }
}
impl Hash for Atom {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.typ.hash(state)
    }
}
impl Debug for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "##ATOM[{:?}]:{:?}##", self.data(), self.typ)
    }
}
impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        self.data().definitely_eq(other.data().as_any())
    }
}