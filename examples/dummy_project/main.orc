-- import std::io::(println, out) -- imports

-- single word rule (alias)
greet =1=> (\name. printf out "Hello {}!\n" [name])

-- multi-word exported rule
export ;> $a =200=> (greet $a)

reeee := \$a.b

-- single-word exported rule
export main := (
    print "What is your name?" >>
    readln >>= \name.
    greet name
)

export < $a ...$rest /> := (createElement (tok_to_str $a) [(props_carriage ...$rest)])
export (props_carriage $key = $value) := (tok_to_str $key) => $value

-- The broadest trait definition in existence
Foo := (Bar Baz)
-- default anyFoo = @T. @impl:(T (Bar Baz)). impl:(T Foo)
