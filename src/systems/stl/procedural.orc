import super::functional::=>

-- remove duplicate ;-s
export macro do {
  ...$statement ; ; ...$rest:1
} =0x3p130=> do { 
  ...$statement ; ...$rest
}
-- modular operation block that returns a value
export macro do {
  ...$statement ; ...$rest:1
} =0x2p130=> statement (...$statement) (do { ...$rest })
export macro do { ...$return } =0x1p130=> (...$return)
-- modular operation block that returns a CPS function
export macro do cps { ...$body } =0x1p130=> \cont. do { ...$body ; cont }

export macro statement (let $name = ...$value) (...$next) =0x1p230=> (
  ( \$name. ...$next) (...$value)
)
export macro statement (cps ...$names = ...$operation:1) (...$next) =0x2p230=> (
  (...$operation) ( (...$names) => ...$next )
)
export macro statement (cps ...$operation) (...$next) =0x1p230=> (
  (...$operation) (...$next)
)
