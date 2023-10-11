import std::panic

export const some := \v. \d. \f. f v
export const none := \d. \f. d

export const map := \option. \f. option none f
export const flatten := \option. option none \opt. opt
export const flatmap := \option. \f. option none \opt. map opt f
export const unwrap := \option. option (panic "value expected") \x.x
