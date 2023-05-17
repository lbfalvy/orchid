---
marp: true
class: invert
---

# Orchid

some tagline

---

## Syntax

basically lambda calc
```
half := \n. div n 2
pair := \a.\b. \f. f a b
increment := add 1
```

---

## Macros

match and replace token sequences
```
if ...$cond then ...$true else ...$false ==> (ifthenelse (...$cond) (...$true) (...$false))
```
...while keeping parameters intact
```
$data -- a single token (including parenthesized sequences)
...$data -- at least one token
..$data -- zero or more tokens
```

---

## Macros

define operators...
```
...$a + ...$b ==> (add (...$a) (...$b))
```
...and named bindings...
```
let $name = ...$value in ...$body ==> (\$name. ...$body) ...$value
```
...and control structures
```
loop $r on (...$parameters) with ...$tail ==> Y (\$r.
  bind_names (...$parameters) (...$tail)
) ...$parameters

-- bind each of the names in the first argument as a parameter for the second argument
bind_names ($name ..$rest) $payload ==> \$name. bind_names (..$rest) $payload
bind_names () (...$payload) ==> ...$payload
```

---

## Macros

can expose interfaces...
```
do { ...$statement ; ...$rest } ==> (statement (...$statement) do { ...$rest })
do { ...$return } ==> (...$return)
```
...to be used by others...
```
statement (let $name = ...$value) ...$next ==> ((\$name. ...$next) (...$value))
statement (cps $name = ...$operation) ...$next ==> ((...$operation) \$name. ...$next)
statement (cps ...$operation) ...$next ==> ((...$operation) (...$next))
```
...to define any syntax
```
export main := do{
  cps data = readline;
  let double = parse_float data * 2;
  cps print (to_string double ++ "\n")
}
```

---

## Control

remains with the embedder

|             |     extension      |      supervision       |
| ----------: | :----------------: | :--------------------: |
|    pipeline | external libraries |  file IO interception  |
|      macros |                    | step-by-step execution |
| interpreter |  constants, input  |          gas           |

---

## Extensions

```rs
use std::fmt::Debug;
use crate::external::litconv::with_lit;
use crate::representations::{interpreted::ExprInst, Literal};
use crate::{atomic_impl, atomic_redirect, externfn_impl};

#[derive(Clone)]
pub struct ToString1;
externfn_impl!(ToString1, |_: &Self, x: ExprInst| Ok(ToString0{x}));

#[derive(Debug, Clone)]
pub struct ToString0{ x: ExprInst }
atomic_redirect!(ToString0, x);
atomic_impl!(ToString0, |Self{ x }: &Self, _| {
  let string = with_lit(x, |l| Ok(match l {
    Literal::Char(c) => c.to_string(),
    Literal::Uint(i) => i.to_string(),
    Literal::Num(n) => n.to_string(),
    Literal::Str(s) => s.clone()
  }))?;
  Ok(string.into())
});
```