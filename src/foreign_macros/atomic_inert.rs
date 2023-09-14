#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;

#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;

/// Implement [Atomic] for a structure that cannot be transformed any further.
/// This would be optimal for atomics encapsulating raw data. [Atomic] depends
/// on [Any], [Debug] and [DynClone].
/// 
/// If the type in question is parametric, the angle brackets must be replaced
/// by parentheses, and the contraints must be parenthesized, for conenient
/// parsing. See the below example:
/// 
/// ```ignore
/// use orchidlang::atomic_inert;
/// 
/// struct MyContainer<T, U: Clone, V: Eq + Hash>()
/// 
/// atomic_inert!( MyContainer(T, U:(Clone), V:(Eq + Hash)), "my container" );
/// ```
#[macro_export]
macro_rules! atomic_inert {
  ( $typ:ident $( (
    $( $typevar:ident $( : (
      $( $constraints:tt )*
  ) )? ),+ ) )?
  , typestr = $typename:expr $( , request = $reqhandler:expr )?) => {
    impl $(< $($typevar : $( $($constraints)* + )? 'static ),+ >)? $crate::foreign::Atomic
    for $typ $(< $($typevar),+ >)? {
      $crate::atomic_defaults! {}

      fn run(
        &self,
        ctx: $crate::interpreter::Context,
      ) -> $crate::foreign::AtomicResult {
        Ok($crate::foreign::AtomicReturn {
          clause: self.clone().atom_cls(),
          gas: ctx.gas,
          inert: true,
        })
      }

      $(
        fn request(
          &self,
          request: Box<dyn std::any::Any>
        ) -> Option<Box<dyn std::any::Any>> {
          let lambda = $reqhandler;
          lambda(request, self)
        }
      )?
    }

    impl $(< $($typevar : $( $($constraints)* + )? 'static ),+ >)?
      TryFrom<&$crate::interpreted::ExprInst>
    for $typ $(< $($typevar),+ >)? {
      type Error = std::rc::Rc<dyn $crate::foreign::ExternError>;

      fn try_from(
        value: &$crate::interpreted::ExprInst,
      ) -> Result<Self, Self::Error> {
        $crate::systems::cast_exprinst::with_atom(
          value,
          $typename,
          |a: &$typ $(< $($typevar),+ >)?| Ok(a.clone()),
        )
      }
    }
  };
}
