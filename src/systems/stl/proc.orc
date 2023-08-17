import super::fn::=>

-- remove duplicate ;-s
export macro do {
  ...$statement ; ; ...$rest:1
} =0x3p130=> do { 
  ...$statement ; ...$rest
}
export macro do {
  ...$statement ; ...$rest:1
} =0x2p130=> statement (...$statement) do { ...$rest }
export macro do { ...$return } =0x1p130=> ...$return

export macro statement (let $name = ...$value) ...$next =0x1p230=> (
  ( \$name. ...$next) (...$value)
)
export macro statement (cps ...$names = ...$operation:1) ...$next =0x2p230=> (
  (...$operation) ( (...$names) => ...$next )
)
export macro statement (cps ...$operation) ...$next =0x1p230=> (
  (...$operation) (...$next)
)
