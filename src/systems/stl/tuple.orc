import super::(known::*, bool::*, number::*)

const discard_args := \n. \value. (
  if n == 0 then value
  else \_. discard_args (n - 1) value
)

export const pick := \tuple. \i. \n. tuple (
  discard_args i \val. discard_args (n - 1 - i) val
)

macro t[...$item, ...$rest:1] =0x2p84=> (\f. t[...$rest] (f (...$item)))
macro t[...$end] =0x1p84=> (\f. f (...$end))
macro t[] =0x1p84=> \f.f

export ::(t)
