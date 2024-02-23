import std::(panic, string::char_at)
import std::conv::(to_float, to_string)

export const main := do{
  cps data = prompt "left operand: ";
  let a = to_float data;
  cps op = prompt "operator: ";
  cps println "you selected \"${op}\"";
  cps data = prompt "right operand: ";
  let b = to_float data;
  let result = (
    if op == "+" then a + b
    else if op == "-" then a - b
    else if op == "*" then a * b
    else if op == "/" then a / b
    else (panic "Unsupported operation")
  );
  cps println "Result: ${result}";
  exit_status::success
}
