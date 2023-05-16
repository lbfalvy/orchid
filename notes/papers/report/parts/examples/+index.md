# Examples

The following examples all work in the submitted version of Orchid, they're included in various subdircetories of `examples`.

## Prelude

All code files implicitly include the head statement

```
import prelude::*
```

The `prelude` module is a string literal compiled into the interpreter. Its contents are as follows:

```rs
static PRELUDE_TXT:&str = r#"
import std::(
  add, subtract, multiply, remainder, divide,
  equals, ifthenelse,
  concatenate
)

export ...$a + ...$b =1001=> (add (...$a) (...$b))
export ...$a - ...$b:1 =1001=> (subtract (...$a) (...$b))
export ...$a * ...$b =1000=> (multiply (...$a) (...$b))
export ...$a % ...$b:1 =1000=> (remainder (...$a) (...$b))
export ...$a / ...$b:1 =1000=> (divide (...$a) (...$b))
export ...$a == ...$b =1002=> (equals (...$a) (...$b))
export ...$a ++ ...$b =1003=> (concatenate (...$a) (...$b))

export do { ...$statement ; ...$rest:1 } =0x2p543=> (
  statement (...$statement) do { ...$rest } 
)
export do { ...$return } =0x1p543=> (...$return)

export statement (let $name = ...$value) ...$next =0x1p1000=> (
  (\$name. ...$next) (...$value)
)
export statement (cps $name = ...$operation) ...$next =0x2p1000=> (
  (...$operation) \$name. ...$next
)
export statement (cps ...$operation) ...$next =0x1p1000=> (
  (...$operation) (...$next)
)

export if ...$cond then ...$true else ...$false:1 =0x1p320=> (
  ifthenelse (...$cond) (...$true) (...$false)
)

export ::(,)
"#;
```

The meaning of each of these rules is explained in the [calculator example](./calculator.md). The exact file is included here just as a reference while reading the other examples.