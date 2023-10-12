import system::async::(set_timer, yield)
import system::io::(readln, println)

const main := (
  set_timer true 1 (println "y" yield)
    \cancel. readln \a. cancel
)
