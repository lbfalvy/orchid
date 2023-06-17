import std::(proc::*, io::print, to_string)

export main := do{
  let foo = list::new[1, 2, 3, 4, 5, 6];
  let bar = list::map foo n => n * 2;
  let sum = bar
    |> list::skip 2
    |> list::take 3
    |> list::reduce 0 (a b) => a + b;
  cps print $ to_string sum ++ "\n";
  0
}