import super::fn::=>

-- remove duplicate ;-s
export do { ...$statement ; ; ...$rest:1 } =0x3p130=> do { ...$statement ; ...$rest }
export do { ...$statement ; ...$rest:1 } =0x2p130=> statement (...$statement) do { ...$rest }
export do { ...$return } =0x1p130=> ...$return

export statement (let $name = ...$value) ...$next =0x1p230=> (
  ( \$name. ...$next) (...$value)
)
export statement (cps ...$names = ...$operation:1) ...$next =0x2p230=> (
  (...$operation) ( (...$names) => ...$next )
)
export statement (cps ...$operation) ...$next =0x1p230=> (
  (...$operation) (...$next)
)