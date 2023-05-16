import list
import map
import option
import fn::*

export main := do{
  let foo = map::new[
    "foo" = 1,
    "bar" = 2,
    "baz" = 3,
    "bar" = 4
  ];
  map::get foo "bar"
    |> option::unwrap
}

--[
export main := do{
  let foo = list::new[1, 2, 3];
  map::fst foo
}
]--