/// A variation on [take_mut::take] that allows the callback to return a value
pub fn take_with_output<T, U>(src: &mut T, cb: impl FnOnce(T) -> (T, U)) -> U {
  take_mut::scoped::scope(|scope| {
    let (old, hole) = scope.take(src);
    let (new, out) = cb(old);
    hole.fill(new);
    out
  })
}
