import std::conv::(parse_float, to_string)
import std::cpsio::(readline, print)
import std::str::(concatenate)

export main := do{
  cps data = readline;
  let a = parse_float data;
  cps op = readline;
  cps print ("\"" ++ op ++ "\"\n");
  cps data = readline;
  let b = parse_float data;
  let result = (
    if op = "+" then a + b
    else if op = "-" then a - b
    else if op = "*" then a * b
    else if op = "/" then a / b
    else "Unsupported operation" -- dynamically typed shenanigans
  );
  cps print (to_string result ++ "\n");
  0
}
