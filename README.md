Orchid will be a compiled functional language with a powerful macro
language and optimizer.

# Examples

Hello World in Orchid
```orchid
import std::io::(println, out)

main := println out "Hello World!"
```

Basic command line calculator
```orchid
import std::io::(readln, printf, in, out)

main := (
    readln in >>= int |> \a. 
    readln in >>= \op.
    readln in >>= int |> \b.
    printf out "the result is {}\n", [match op (
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b
    )]
)
```

Grep
```orchid
import std::io::(readln, println, in, out, getarg)

main := loop \r. (
    readln in >>= \line.
    if (substring (getarg 1) line)
    then (println out ln >>= r)
    else r
)
```

Filter through an arbitrary collection
```orchid
filter := @C:Type -> Type. @:Map C. @T. \f:T -> Bool. \coll:C T. (
    coll >> \el. if (f el) then (Some el) else Nil
):(C T)
```

# Explanation

This explanation is not a tutorial. It follows a constructive order,
gradually introducing language features to better demonstrate their
purpose. It also assumes that the reader is familiar with functional
programming.

## Lambda calculus recap

The language is almost entirely based on lambda calculus, so everything
is immutable and evaluation is lazy. The following is an anonymous
function that takes an integer argument and multiplies it by 2:

```orchid
\x:int. imul 2 x
```

Multiple parameters are represented using currying, so the above is
equivalent to

```orchid
imul 2
```

Recursion is accomplished using the Y combinator (called `loop`), which
is a function that takes a function as its single parameter and applies
it to itself. A naiive implementation of `imul` might look like this.

```orchid
\a:int.\b:int. loop \r. (\i.
    ifthenelse (ieq i 0)
        b
        (iadd b (r (isub i 1))
) a
```

`ifthenelse` takes a boolean as its first parameter and selects one of the
following two expressions (of identical type) accordingly. `ieq`, `iadd`
and `isub` are self explanatory.

## Auto parameters (generics, polymorphism)

Although I didin't specify the type of `i` in the above example, it is
known at compile time because the recursion is applied to `a` which is an
integer. I could have omitted the second argument, then I would have
had to specify `i`'s type as an integer, because for plain lambda
expressions all types have to be statically known at compile time.

Polymorphism is achieved using parametric constructs called auto
parameters. An auto parameter is a placeholder filled in during
compilation, syntactically remarkably similar to lambda expressions:

```orchid
@T. --[ body of expression referencing T ]--
```

Autos have two closely related uses. First, they are used to represent
generic type parameters. If an auto is used as the type of an argument
or some other subexpression that can be trivially deduced from the calling
context, it is filled in.

The second usage of autos is for constraints, if they have a type that
references another auto. Because these parameters are filled in by the
compiler, referencing them is equivalent to the statement that a default
value assignable to the specified type exists. Default values are declared
explicitly and identified by their type, where that type itself may be
parametric and may specify its own constraints which are resolved
recursively. If the referenced default is itself a useful value or
function you can give it a name and use it as such, but you can also omit
the name, using the default as a hint to the compiler to be able to call
functions that also have defaults of the same types, or possibly other
types whose defaults have implmentations based on your defaults.

For a demonstration, here's a sample implementation of the Option monad.
```orchid
--[[ The definition of Monad ]]--
Bind := \M:Type -> Type. @T -> @U -> (T -> M U) -> M T -> M U
Return := \M:Type -> Type. @T -> T -> M T
Monad := \M:Type -> Type. (
    @:Bind M.
    @:Return M.
    0 --[ Note that empty expressions are forbidden so those that exist
        purely for their constraints should return a nondescript constant
        that is likely to raise a type error when used by mistake, such as
        zero ]--
)

--[[ The definition of Option ]]--
export Option := \T:Type. @U -> U -> (T -> U) -> U
--[ Constructors ]--
export Some := @T. \data:T. ( \default. \map. map data ):(Option T)
export None := @T.          ( \default. \map. default  ):(Option T)
--[ Implement Monad ]--
default returnOption := Some:(Return Option)
default bindOption := ( @T:Type. @U:Type.
    \f:T -> U. \opt:Option T. opt None f
):(Bind Option)
--[ Sample function that works on unknown monad to demonstrate HKTs.
    Turns (Option (M T)) into (M (Option T)), "raising" the unknown monad
    out of the Option ]--
export raise := @M:Type -> Type. @T:Type. @:Monad M. \opt:Option (M T). (
    opt (return None) (\m. bind m (\x. Some x))
):(M (Option T))
```

Defaults may be defined in any module that also defines at least one of
the types in the definition, which includes both the type of the
expression and the types of its auto parameters. They always have a name,
which can be used to override known defaults with which your definiton
may overlap. For example, if addition is defined elementwise for all
applicative functors, the author of List might want for concatenation to
take precedence in the case where all element types match. Notice how
Add has three arguments, two are the types of the operands and one is
the result:

```orchid
default concatListAdd replacing elementwiseAdd := @T. (
    ...
):(Add (List T) (List T) (List T))
```

For completeness' sake, the original definition might look like this:

```orchid
default elementwiseAdd := @C:Type -> Type. @T. @U. @V. @:(Applicative C). @:(Add T U V). (
    ...
):(Add (C T) (C U) (C V))
```

With the use of autos, here's what the recursive multiplication
implementation looks like:

```orchid
default iterativeMultiply := @T. @:(Add T T T). (
    \a:int.\b:T. loop \r. (\i.
        ifthenelse (ieq i 0)
            b
            (add b (r (isub i 1)) -- notice how iadd is now add
    ) a
):(Multiply T int T)
```

This could then be applied to any type that's closed over addition

```orchid
aroundTheWorldLyrics := (
    mult 18 (add (mult 4 "Around the World\n") "\n")
)
```

## Preprocessor

The above code samples have one notable difference from the Examples
section above; they're ugly and hard to read. The solution to this is a
powerful preprocessor which is used internally to define all sorts of
syntax sugar from operators to complex syntax patterns and even pattern
matching, and can also be used to define custom syntax. The preprocessor
reads the source as an S-tree while executing substitution rules which
have a real numbered priority.

In the following example, seq matches a list of arbitrary tokens and its
parameter is the order of resolution. The order can be used for example to
make sure that `if a then b else if c then d else e` becomes
`(ifthenelse a b (ifthenelse c d e))` and not
`(ifthenelse a b if) c then d else e`. It's worth highlighting here that
preprocessing works on the typeless AST and matchers are constructed
using inclusion rather than exclusion, so it would not be possible to
selectively allow the above example without enforcing that if-statements
are searched back-to-front. If order is still a problem, you can always
parenthesize subexpressions at the callsite.

```orchid
(..$pre:2 if $1 then $2 else $3 ..$post:1) =2=> (
    ..$pre
    (ifthenelse $1 $2 $3)
    ...$post
)
$a + $b =10=> (add $a $b)
$a = $b =5=> (eq $a $b)
$a - $b =10=> (sub $a $b)
```

The recursive addition function now looks like this

```orchid
default iterativeMultiply := @T. @:(Add T T T). (
    \a:int.\b:T. loop \r. (\i.
        if (i = 0) then b
        else (b + (r (i - 1)))
    ) a
):(Multiply T int T)
```

### Traversal using carriages

While it may not be immediately apparent, these substitution rules are
actually Turing complete. They can be used quite intuitively to traverse
the token tree with unique "carriage" symbols that move according to their
environment and can carry structured data payloads.

Here's an example of a carriage being used to turn a square-bracketed
list expression into a lambda expression that matches a conslist. Notice
how the square brackets pair up, as all three variants of brackets
are considered branches in the S-tree rather than individual tokens.

```orchid
-- Initial step, eliminates entry condition (square brackets) and constructs
-- carriage and other working symbols
[...$data:1] =1000.1=> (cons_start ...$data cons_carriage(none))
-- Shortcut with higher priority
[] =1000.5=> none
-- Step
, $item cons_carriage($tail) =1000.1=> cons_carriage((some (cons $item $tail)))
-- End, removes carriage and working symbols and leaves valid source code
cons_start $item cons_carriage($tail) =1000.1=> some (cons $item $tail)
-- Low priority rules should turn leftover symbols into errors.
cons_start =0=> cons_err
cons_carriage($data) =0=> cons_err
cons_err =0=> (macro_error "Malformed conslist expression")
-- macro_error will probably have its own rules for composition and
-- bubbling such that the output for an erratic expression would be a
-- single macro_error to be decoded by developer tooling
```
(an up-to-date version of this example can be found in the examples
folder)

Another thing to note is that although it may look like cons_carriage is
a global string, it's in fact namespaced to whatever file provides the
macro. Symbols can be exported either by prefixing the pattern with
`export` or separately via the following syntax if no single rule is
equipped to dictate the exported token set.

```orchid
export ::(some_name, other_name)
```

# Module system

Files are the smallest unit of namespacing, automatically grouped into
folders and forming a tree the leaves of which are the actual symbols. An
exported symbol is a name referenced in an exported substitution rule
or assigned to an exported function. Imported symbols are considered
identical to the same symbol directly imported from the same module for
the purposes of substitution. The module syntax is very similar to
Rust's, and since each token gets its own export with most rules
comprising several local symbols, the most common import option is
probably ::* (import all).

# Optimization

This is very far away so I don't want to make promises, but I have some
ideas. 

- [ ] early execution of functions on any subset of their arguments where
    it could provide substantial speedup
- [ ] tracking copies of expressions and evaluating them only once
- [ ] Many cases of single recursion converted to loops
    - [ ] tail recursion
    - [ ] 2 distinct loops where the tail doesn't use the arguments
        - [ ] reorder operations to favour this scenario 
- [ ] reactive calculation of values that are deemed to be read more often
    than written
- [ ] automatic profiling based on performance metrics generated by debug
    builds
