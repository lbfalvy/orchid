import super::bool::*

export ::(+, -, [*], %, /, <, >, <=, >=)

const less_than_or_equal := \a. \b. a < b or a == b

macro ...$a + ...$b =0x2p36=> (add (...$a) (...$b))
macro ...$a:1 - ...$b =0x2p36=> (subtract (...$a) (...$b))
macro ...$a * ...$b =0x1p36=> (multiply (...$a) (...$b))
macro ...$a:1 % ...$b =0x1p36=> (remainder (...$a) (...$b))
macro ...$a:1 / ...$b =0x1p36=> (divide (...$a) (...$b))
macro ...$a:1 < ...$b =0x3p36=> (less_than (...$a) (...$b))
macro ...$a:1 > ...$b =0x3p36=> ((...$b) < (...$a))
macro ...$a:1 <= ...$b =0x3p36=> (less_than_or_equal (...$a) (...$b))
macro ...$a:1 >= ...$b =0x3p36=> ((...$b) <= (...$a))

