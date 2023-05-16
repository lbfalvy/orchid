import std::panic

export some := \v. \d.\f. f v
export none := \d.\f. d

export map := \option.\f. option none f
export flatten := \option. option none \opt. opt
export flatmap := \option.\f. option none \opt. map opt f
export unwrap := \option. option (panic "value expected") \x.x