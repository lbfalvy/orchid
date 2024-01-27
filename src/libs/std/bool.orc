import std::(pmatch, inspect)

export ::(!=, ==)

export const not := \bool. if bool then false else true
macro ...$a != ...$b =0x3p36=> (not (...$a == ...$b))
macro ...$a == ...$b =0x3p36=> (equals (...$a) (...$b))
export macro ...$a and ...$b =0x4p36=> (ifthenelse (...$a) (...$b) false)
export macro ...$a or ...$b =0x4p36=> (ifthenelse (...$a) true (...$b))
export macro if ...$cond then ...$true else ...$false:1 =0x1p84=> (
  ifthenelse (...$cond) (...$true) (...$false)
)

(
  macro pmatch::request (== ...$other)
  =0x1p230=> pmatch::response (
    if pmatch::value == (...$other)
    then pmatch::pass
    else pmatch::fail
  )
  ( pmatch::no_binds )
)

(
  macro pmatch::request (!= ...$other)
  =0x1p230=> pmatch::response (
    if pmatch::value != (...$other)
    then pmatch::pass
    else pmatch::fail
  )
  ( pmatch::no_binds )
)

(
  macro pmatch::request (true)
  =0x1p230=> pmatch::response
    (if pmatch::value then pmatch::pass else pmatch::fail)
    ( pmatch::no_binds )
)

(
  macro pmatch::request (false)
  =0x1p230=> pmatch::response
    (if pmatch::value then pmatch::fail else pmatch::pass)
    ( pmatch::no_binds )
)
