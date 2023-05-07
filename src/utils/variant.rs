// trait Var<T> {
//   type With<U>: Var<U>;

//   fn map<U>(self, f: impl FnOnce(T) -> U) -> Self::With<U>;
//   fn map_multi<U, V, Ret: Var<U> + Var<V>>(
//     self, f: impl FnOnce(T) -> Ret
//   ) -> <Self::With<U> as Var<U>>::With<V>;
// }

// enum Variant<T, U> {
//   Head(T),
//   Tail(U)
// }

// impl<H, T: Var<_>> Var<H> for Variant<H, T> {
//   fn map<U>(self, f: impl FnOnce(H) -> U) -> Self::With<U> {
//     match 
//   }
// }
