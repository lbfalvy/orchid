import std::panic

as_type ()

export const ok := \v. wrap \fe. \fv. fv v
export const err := \e. wrap \fe. \fv. fe e

export const map := \result. \fv. unwrap result err fv
export const map_err := \result. \fe. unwrap result fe ok
export const flatten := \result. unwrap result err \res. wrap (unwrap res)
export const and_then := \result. \f. unwrap result err \v. f v
export const assume := \result. unwrap result (\e. panic "value expected") \v.v
