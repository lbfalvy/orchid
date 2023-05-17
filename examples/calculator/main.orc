import std::(parse_float, to_string)
import std::(readline, print)

export main := do{
  cps print "left operand: ";
  cps data = readline;
  let a = parse_float data;
  cps print "operator: ";
  cps op = readline;
  cps print ("you selected \"" ++ op ++ "\"\n");
  cps print "right operand: ";
  cps data = readline;
  let b = parse_float data;
  let result = (
    if op == "+" then a + b
    else if op == "-" then a - b
    else if op == "*" then a * b
    else if op == "/" then a / b
    else (panic "Unsupported operation")
  );
  cps print ("Result: " ++ to_string result ++ "\n");
  0
}