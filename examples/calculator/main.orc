import std::(proc::*, to_float, to_string, panic, str::char_at)

export const main := do{
  cps print "left operand: ";
  cps data = readln;
  let a = to_float data;
  cps print "operator: ";
  cps op = readln;
  let op = char_at op 0;
  cps println ("you selected \"" ++ op ++ "\"");
  cps print "right operand: ";
  cps data = readln;
  let b = to_float data;
  let result = (
    if op == "+" then a + b
    else if op == "-" then a - b
    else if op == "*" then a * b
    else if op == "/" then a / b
    else (panic "Unsupported operation")
  );
  cps println ("Result: " ++ to_string result);
  0
}
