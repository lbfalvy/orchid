import std::(panic, pmatch, to_string, conv)
import std::(functional::*, string::*)

as_type option (
  impl to_string := \opt. (
    handle opt "none" \x. "some(" ++ conv::to_string x ++ ")"
  )
)

export const some := \v. wrap \d. \f. f v
export const none := wrap \d. \f. d

export const handle := \t. \d. \f. (unwrap t) d f

export const map := \option. \f. handle option none \x. some $ f x
export const fallback := \option. \fallback. handle option fallback \data. data
export const flatten := \option. handle option none \opt. wrap $ unwrap opt -- assert type
export const flatmap := \option. \f. handle option none \opt. wrap $ unwrap $ f opt -- assert return
export const assume := \option. handle option (panic "value expected") \x.x

(
  macro pmatch::request ( none )
  =0x1p230=> pmatch::response (
    handle pmatch::value
      pmatch::pass
      \_. pmatch::fail
  ) ( pmatch::no_binds )
)

(
  macro pmatch::request ( some ...$value )
  =0x1p230=> await_some_subpattern ( pmatch::request (...$value) )
)

(
  macro await_some_subpattern ( pmatch::response $expr ( $binds ) )
  =0x1p254=> pmatch::response (
    handle pmatch::value
      pmatch::fail
      \pmatch::value. $expr
  ) ( $binds )
)
