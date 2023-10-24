import std::to_string

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
  map::having ["is_alive" = true, "greeting" = foo] => foo
}

const main := test2 ++ ", " ++ test1
