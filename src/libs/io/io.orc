import std::panic
import system::io
import system::async::yield

export const print := \text. \ok. (
  io::write_str io::stdout text
    (io::flush io::stdout
      ok
      (\e. panic "println threw on flush")
      \_. yield
    )
    (\e. panic "print threw on write")
    \_. yield
)

export const println := \line. \ok. (
  print (line ++ "\n") ok
)

export const readln := \ok. (
  io::read_line io::stdin
    ok
    (\e. panic "readln threw")
    \_. yield
)

export const prompt := \line. \ok. (
  print line (readln ok)
)

export module prelude (
  import super::*

  export ::(print, println, readln, prompt)
)
