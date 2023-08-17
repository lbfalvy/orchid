import std::(proc::*, to_string)

export const main := do{
  let foo = map::new[
    "foo" = 1,
    "bar" = 2,
    "baz" = 3,
    "bar" = 4
  ];
  let num = map::get foo "bar"
    |> option::unwrap;
  cps println $ to_string num;
  0
}
