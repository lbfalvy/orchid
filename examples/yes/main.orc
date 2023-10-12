import system::async::(set_timer, yield)
import system::io::(readln, println)
import std::exit_status

const main := do{
  cps cancel = set_timer true 1 (println "y" yield);
  cps _ = readln;
  cps cancel;
  exit_status::success
}
