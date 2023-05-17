import option
import super::fn::*

pair := \a.\b. \f. f a b

-- Constructors

export cons := \hd.\tl. option::some (pair hd tl)
export end := option::none

export pop := \list.\default.\f.list default \cons.cons f

-- Operators

export reduce := \list.\acc.\f. (
  loop r on (list acc) with
    pop list acc \head.\tail. r tail (f acc head)
)

export map := \list.\f. (
  loop r on (list) with
    pop list end \head.\tail. cons (f head) (r tail)
)

export skip := \list.\n. (
  loop r on (list n) with
    if n == 0 then list
    else pop list end \head.\tail. r tail (n - 1)
)

export take := \list.\n. (
  loop r on (list n) with
    if n == 0 then end
    else pop list end \head.\tail. cons head $ r tail $ n - 1
)

new[...$item, ...$rest:1] =0x2p84=> (cons (...$item) new[...$rest])
new[...$end] =0x1p84=> (cons (...$end) end)
new[] =0x1p84=> end

export ::(new)
