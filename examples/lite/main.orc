import std::(parse_float, to_string)
import std::(readline, print, debug)
import std::(concatenate)
import super::list
import fn::*

--[ export main := do{
  cps data = readline;
  let a = parse_float data;
  cps op = readline;
  cps print ("\"" ++ op ++ "\"\n");
  cps data = readline;
  let b = parse_float data;
  let result = (
    if op == "+" then a + b
    else if op == "-" then a - b
    else if op == "*" then a * b
    else if op == "/" then a / b
    else "Unsupported operation" -- dynamically typed shenanigans
  );
  cps print (to_string result ++ "\n");
  0
} ]--

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

--[
export main := do{
  let n = 1;
  let acc = 1;
  loop r on (n acc) with (
    if n == 5
    then print acc
    else r (n + 1) (acc * 2)
  )
}
]--
--[
export main := do{
  let n = 1;
  loop r on (n) with (
    debug r
  )
}
]--