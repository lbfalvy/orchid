import list
import map
import option
import fn::*
import std::(print, to_string)

export main := do{
  let foo = map::new[
    "foo" = 1,
    "bar" = 2,
    "baz" = 3,
    "bar" = 4
  ];
  let num = map::get foo "bar"
    |> option::unwrap;
  cps print (to_string num ++ "\n");
  0
}