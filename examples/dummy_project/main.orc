import std::io::(println, out) -- imports

-- single word substitution (alias)
greet = \name. printf out "Hello {}!\n" [name]

-- multi-word exported substitution
export (...$pre ;) $a ...$post) =200=> (...$pre (greet $a) ...$post)

-- single-word exported substitution
export main = (
    print "What is your name? >>
    readln >>= \name.
    greet name
)

-- The broadest trait definition in existence
Foo = Bar Baz
default anyFoo = @T. @impl:(T (Bar Baz)). impl:(T Foo)