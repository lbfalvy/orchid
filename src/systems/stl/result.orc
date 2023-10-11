import std::panic

export const ok := \v. \fe. \fv. fv v
export const err := \e. \fe. \fv. fe e

export const map := \result. \fv. result err fv
export const map_err := \result. \fe. result fe ok
export const flatten := \result. result err \res. res
export const and_then := \result. \f. result err \v. f v
export const unwrap := \result. result (\e. panic "value expected") \v.v
