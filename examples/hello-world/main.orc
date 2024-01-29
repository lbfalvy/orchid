import std::exit_status
import std::conv
import std::number
import std::tuple
import std::list

const main := match t["set", "foo", 1] {
  t[= "set", key, val] => 
    $"Setting ${ key ++ $"${1 + 1}" } to ${val}"
}
