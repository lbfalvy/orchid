import system::async::(set_timer, yield)
import system::io::(readln, println)
import std::exit_status

const main := (
  set_timer true 1 (println "y" yield) \cancel.
    readln \a.
      cancel exit_status::success
)
