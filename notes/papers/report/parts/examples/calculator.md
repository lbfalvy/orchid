# Calculator

This example demonstrates various parts of the standard library, infix operators, `do{}` blocks, and various syntax elements. Approching MVP, this was the first benchmark created to debug various features. It predates the transition for `:=` from single-token macros to a dedicated language element.

```
import std::(parse_float, to_string)
import std::(readline, print)

export main := do{
  cps data = readline;
  let a = parse_float data;
  cps op = readline;
  cps print ("\"" ++ op ++ "\"\n");
  cps data = readline;
  let b = parse_float data;
  let result = (
    if op == "+" then a + b
    else if op == "-" then a - b
    else if op == "*" then a * b
    else if op == "/" then a / b
    else "Unsupported operation" -- dynamically typed shenanigans
  );
  cps print (to_string result ++ "\n");
  0
}
```

## do

The main function uses a `do{}` block, which is processed using the following rules, temporarily added to the prelude:

```
export do { ...$statement ; ...$rest:1 } =0x2p543=> (
  statement (...$statement) do { ...$rest } 
)
export do { ...$return } =0x1p543=> (...$return)
```

This pair of rules converts the flat structure into a conslist which makes it easier for dedicated statement rules to process their own fragments. The produced structure looks roughly like this:

```
(statement (cps data = readline)
(statement (let a = parse_float data)
(statement (cps op = readline)
( ...
(statement (cps print (to_string result ++ "\n"))
(0)
)))))
```

`do` blocks contain semicolon-delimited statements which receive special handling, and a final expression that doesn't. This final expression must be present since every Orchid expression must produce a value including `do` blocks. For ergonomics, in the future a sentinel value may be returned if the body of the `do` block ends with a semicolon.

## statement

This example demonstrates three statement types. This collection can be extended by matching on `prelude::statement (<custom statement syntax>) ...$next`.

### let

`let` bindings are used for forward-declaring values in subsequent expressions, passing them to the rest of the body.
```
export statement (let $name = ...$value) ...$next =0x1p1000=> (
  (\$name. ...$next) (...$value)
)
```

Since the executor keeps track of copies of the same expression and applies normalization steps to a shared instance, this technique also ensures that `...$value` will not be evaluated multiple times.

### cps=

`cps` was used for effectful functions.
```
export statement (cps $name = ...$operation) ...$next =0x2p1000=> (
  (...$operation) \$name. ...$next
)
```

In the version of Orchid this example was written for, functions like `print` or `readline` carried out their work as a side effect of normalization. At this point the copy-tracking optimization described above wasn't used. Because of this, in new versions `print` or `readline` in a loop doesn't necessarily repeat its effect. This bug can be addressed in the standard library, but `cps` would still probably be just as useful.

### cps

Since `cps` is designed for side effects, an expression of this kind doesn't necessarily produce a value. This `=` free variant passes the tail as an argument to the expression as-is
```
export statement (cps ...$operation) ...$next =0x1p1000=> (
  (...$operation) (...$next)
)
```

## if-then-else

This rule is substantially simpler, it simply forwards the three slots to a function that makes the actual decision.
```
export if ...$cond then ...$true else ...$false:1 =0x1p320=> (
  ifthenelse (...$cond) (...$true) (...$false)
)
```

Notice that `else if` isn't a syntax element, it's simply an artifact of this rule applied to itself. The critical ordering requirement that enables this is that `cond` and `true` are squeezed so neither of them can accidentally consume an `if` or `else` token. `::prefix:0` is implied at the start, it is left of `cond:0` and `true:0` so it has a higher growth priority, and `false:1` has a higher explicit priority.

## Infix operators

Infix operators could be intuitively defined with something like the following

```
$lhs + $rhs =1=> (add $lhs $rhs)
$lhs * $rhs =2=> (mul $lhs $rhs)
```

However, if they really were defined this way, function application would have the lowest priority. Ideally, we would like function application to have the highest priority.
```
-- what we mean
(mult (parse_float "foobar") 2)
-- how we would like to write it
let a = parse_float "foobar" * 2
-- how we would have to write it
let a = (parse_float "foobar") * 2
```

With vectorial placeholders it's possible to define the operators in reverse, i.e. to match the "outermost" operator first.
```
...$lhs + ...$rhs =2=> (add (...$lhs) (...$rhs))
...$lhs * ...$rhs =1=> (mul (...$lhs) (...$rhs))
```

With this, function calls get processed before any operator.

## Dynamically typed shenanigans

If the operator character isn't recognized, `result` gets assigned `"Unsupported operation"`. This wouldn't work in most type systems as `result` is now either a string or a number with no static discriminator. Most of Orchid's functions accept a single type of input with the sole exception being `to_string`.