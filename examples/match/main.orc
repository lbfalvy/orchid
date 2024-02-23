import std::(conv, reflect)

const foo := t[option::some "world!", option::none]

const test1 := match foo { 
  t[option::some balh, option::none] => balh;
}

const bar := map::new[
  "age" = 22,
  "name" = "lbfalvy",
  "is_alive" = true,
  "species" = "human",
  "greeting" = "Hello"
]

const test2 := match bar {
  map::having ["is_alive" = true, "greeting" = hello, "name" = name] => hello
}

const tests := "${test2}, ${test1}"

const main := conv::to_string bar
