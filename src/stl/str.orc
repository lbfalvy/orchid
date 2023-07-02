import super::(proc::*, bool::*, io::panic)

export ...$a ++ ...$b =0x4p36=> (concat (...$a) (...$b))

export char_at := \s.\i. do{
  let slc = slice s i 1;
  if len slc == 1
  then slc
  else panic "Character index out of bounds"
}