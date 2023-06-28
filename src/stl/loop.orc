import super::proc::(;, do, =)
import super::known::*

--[
  Bare fixpoint combinator. Due to its many pitfalls, usercode is
  recommended to use one of the wrappers such as [recursive] or
  [loop_over] instead.
]--
export Y := \f.(\x.f (x x))(\x.f (x x))

--[
  A syntax construct that encapsulates the Y combinator and encourages
  single tail recursion. It's possible to use this for multiple or
  non-tail recursion by using cps statements, but it's more ergonomic
  than [Y] and more flexible than [std::list::fold].

  To break out of the loop, use [std::fn::const] in a cps statement
]--
export loop_over (..$binds) {
  ...$body
} =0x5p129=> Y (\r. 
  def_binds parse_binds (..$binds) do{
    ...$body;
    r apply_binds parse_binds (..$binds)
  }
) init_binds parse_binds (..$binds)

-- parse_binds builds a conslist
parse_binds (...$item, ...$tail:1) =0x2p250=> (
  parse_bind (...$item)
  parse_binds (...$tail)
)
parse_binds (...$item) =0x1p250=> (
  parse_bind (...$item)
  ()
)

-- parse_bind converts items to pairs
parse_bind ($name) =0x1p250=> ($name bind_no_value)
parse_bind ($name = ...$value) =0x1p250=> ($name (...$value))

-- def_binds creates name bindings for everything
def_binds ( ($name $value) $tail ) ...$body =0x1p250=> (
  \$name. def_binds $tail ...$body
)
def_binds () ...$body =0x1p250=> ...$body

-- init_binds passes the value for initializers
init_binds ( ($name bind_no_value) $tail ) =0x2p250=> $name init_binds $tail
init_binds ( ($name $value) $tail ) =0x1p250=> $value init_binds $tail
-- avoid empty templates by assuming that there is a previous token
$fn init_binds () =0x1p250=> $fn

-- apply_binds passes the name for initializers
apply_binds ( ($name $_value) $tail ) =0x1p250=> $name apply_binds $tail
$fn apply_binds () =0x1p250=> $fn

--[
  Alias for the Y-combinator to avoid some universal pitfalls
]--
export recursive $name (..$binds) ...$body =0x5p129=> Y (\$name.
  def_binds parse_binds (..$binds) ...$body
) init_binds parse_binds (..$binds)