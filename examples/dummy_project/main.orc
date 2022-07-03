import std::io::(println, out) -- imports

-- single word rule (alias)
greet =1=> (\name. printf out "Hello {}!\n" [name])

-- multi-word exported rule
export ;> $a =200=> (greet $a)

-- single-word exported rule
export main = (
    print "What is your name?" >>
    readln >>= \name.
    greet name
)

-- The broadest trait definition in existence
Foo = (Bar Baz)
-- default anyFoo = @T. @impl:(T (Bar Baz)). impl:(T Foo)