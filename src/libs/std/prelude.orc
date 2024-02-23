import std::number::*
export ::[+ - * / % < > <= >=]
import std::string::*
export ::[++]
import std::bool::*
export ::([== !=], if, then, else, true, false, and, or, not)
import std::fn::*
export ::([$ |>], identity, pass, pass2, return)
import std::procedural::*
export ::(do, let, cps)
import std::tuple::t
export ::(t)
import std::pmatch::(match, [=>])
export ::(match, [=>])
import std::loop::*
export ::(loop_over, recursive, while)

import std::known::*
export ::[, _ ; . =]

import std::(tuple, list, map, option, exit_status)
export ::(tuple, list, map, option, exit_status)

import std
export ::(std)
