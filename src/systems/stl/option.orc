import std::(panic, match)

export type ty (
  export const some := \v. wrap \d. \f. f v
  export const none := wrap \d. \f. d

  export const handle := \t. \d. \f. (unwrap t) d f
)

export const some := ty::some
export const none := ty::none
export const handle := ty::handle

export const map := \option. \f. handle option none f
export const flatten := \option. handle option none \opt. opt
export const flatmap := \option. \f. handle option none \opt. map opt f
export const unwrap := \option. handle option (panic "value expected") \x.x

(
  macro match::request ( none )
  =0x1p230=> match::response (
    handle match::value
      match::pass
      \_. match::fail
  ) ( match::no_binds )
)

(
  macro match::request ( some ...$value )
  =0x1p230=> await_some_subpattern ( match::request (...$value) )
)

(
  macro await_some_subpattern ( match::response $expr ( $binds ) )
  =0x1p254=> match::response (
    handle match::value
      match::fail
      \match::value. $expr
  ) ( $binds )
)
