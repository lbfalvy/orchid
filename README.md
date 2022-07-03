Orchid will be a compiled functional language with a powerful macro
language and optimizer.

# Examples

Hello World in Orchid
```orchid
import std::io::(println, out)

main = println out "Hello World!"
```

Basic command line calculator
```orchid
import std::io::(readln, printf, in, out)

main = (
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

main = loop \r. (
    readln in >>= \line.
    if (substring (getarg 1) line)
    then (println out ln >>= r)
    else r
)
```

Filter through an arbitrary collection
```orchid
filter = @C:Type -> Type. @:Map C. @T. @U. \f:T -> U. \coll:C T. (
    coll >> \el. if (f el) then (Some el) else Nil
):(C U)
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
Bind = \M:Type -> Type. @T -> @U -> (T -> M U) -> M T -> M U
Return = \M:Type -> Type. @T -> T -> M T
Monad = \M:Type -> Type. (
    @:Bind M.
    @:Return M.
    0 --[ Note that empty expressions are forbidden so those that exist
        purely for their constraints should return a nondescript constant
        that is likely to raise a type error when used by mistake, such as
        zero ]--
)

--[[ The definition of Option ]]--
export Option = \T:Type. @U -> U -> (T -> U) -> U
--[ Constructors ]--
export Some = @T. \data:T. ( \default. \map. map data ):(Option T)
export None = @T.          ( \default. \map. default  ):(Option T)
--[ Implement Monad ]--
default returnOption = Some:(Return Option)
default bindOption = ( @T:Type. @U:Type.
    \f:T -> U. \opt:Option T. opt None f
):(Bind Option)
--[ Sample function that works on unknown monad to demonstrate HKTs.
    Turns (Option (M T)) into (M (Option T)), "raising" the unknown monad
    out of the Option ]--
export raise = @M:Type -> Type. @T:Type. @:Monad M. \opt:Option (M T). (
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
default concatListAdd replacing applicativeAdd = @T. (
    ...
):(Add (List T) (List T) (List T))
```

For completeness' sake, the original definition might look like this:

```orchid
default elementwiseAdd = @C:Type -> Type. @T. @U. @V. @:(Applicative C). @:(Add T U V). (
    ...
):(Add (C T) (C U) (C V))
```

With the use of autos, here's what the recursive multiplication
implementation looks like:

```orchid
default iterativeMultiply = @T. @:(Add T T T). (
    \a:int.\b:T. loop \r. (\i.
        ifthenelse (ieq i 0)
            b
            (add b (r (isub i 1)) -- notice how iadd is now add
    ) a
):(Multiply T int T)
```

This could then be applied to any type that's closed over addition

```orchid
aroundTheWorldLyrics = (
    mult 18 (add (mult 4 "Around the World\n") "\n")
)
```

## Preprocessor

The above code samples have one notable difference from the Examples
section above; they're ugly and hard to read. The solution to this is a
powerful preprocessor which is used internally to define all sorts of
syntax sugar from operators to complex syntax patterns and even pattern
matching, and can also be used to define custom syntax. The preprocessor
executes substitution rules on the S-tree which have a real numbered
priority and an internal order of resolution.

In the following example, seq matches a list of arbitrary tokens and its
parameter is the order of resolution. The order can be used for example to
make sure that `if a then b else if c then d else e` becomes
`(ifthenelse a b (ifthenelse c d e))` and not
`(ifthenelse a b if) c then d else e`. It's worth highlighting here that
preprocessing works on the typeless AST and matchers are constructed
using inclusion rather than exclusion, so it would not be possible to
selectively allow the above example without enforcing that if-statements
are searched back-to-front. If order is still a problem, you can always
parenthesize problematic expressions.

```orchid
(...$pre:(seq 2) if $1 then $2 else $3 ...$post:(seq 1)) =2=> (
    ...$pre
    (ifthenelse $1 $2 $3)
    ...$post
)
$a + $b =10=> (add $a $b)
$a == $b =5=> (eq $a $b)
$a - $b =10=> (sub $a $b)
```

The recursive addition function now looks like this

```orchid
default iterativeMultiply = @T. @:(Add T T T). (
    \a:int.\b:T. loop \r. (\i.
        if (i == 0) then b
        else (b + (r (i - 1)))
    ) a
):(Multiply T int T)
```

### Traversal using carriages

While it may not be immediately apparent, these substitution rules are
actually Turing complete. They can be used quite intuitively to traverse
the token tree with unique "carriage" symbols that move according to their
environment and can carry structured data payloads.

TODO: carriage example

# Module system

Files are the smallest unit of namespacing, automatically grouped into
folders and forming a tree the leaves of which are the actual symbols. An
exported symbol is a name referenced in an exported substitution rule
or assigned to an exported function. Imported symbols are considered
identical to the same symbol directly imported from the same module for
the purposes of substitution.

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