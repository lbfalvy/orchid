import super::pmatch::=>
import super::known::*

export ::(do, statement, [;])

-- remove duplicate ;-s
macro do {
  ...$statement ; ; ...$rest:1
} =0x3p130=> do { 
  ...$statement ; ...$rest
}
-- modular operation block that returns a value
macro do {
  ...$statement ; ...$rest:1
} =0x2p130=> statement (...$statement) (do { ...$rest })
macro do { ...$return } =0x1p130=> (...$return)

export ::let

macro statement (let $_name = ...$value) (...$next) =0x2p230=> (
  ( \$_name. ...$next) (...$value)
)
macro statement (let ...$pattern = ...$value:1) (...$next) =0x1p230=> (
  ( (...$pattern) => (...$next) ) (...$value)
)

export ::cps

-- modular operation block that returns a CPS function
macro do cps { ...$body } =0x1p130=> \cont. do { ...$body ; cont }
macro statement (cps ...$names = ...$operation:1) (...$next) =0x2p230=> (
  (...$operation) ( (...$names) => ...$next )
)
macro statement (cps ...$operation) (...$next) =0x1p230=> (
  (...$operation) (...$next)
)
