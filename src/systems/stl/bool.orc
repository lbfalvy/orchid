export const not := \bool. if bool then false else true
export macro ...$a != ...$b =0x3p36=> (not (...$a == ...$b))
export macro ...$a == ...$b =0x3p36=> (equals (...$a) (...$b))
export macro if ...$cond then ...$true else ...$false:1 =0x1p84=> (
  ifthenelse (...$cond) (...$true) (...$false)
)
