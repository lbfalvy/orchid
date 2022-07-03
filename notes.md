# Anatomy of a code file

```orchid
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
```
