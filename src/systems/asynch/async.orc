import std::panic

export const block_on := \action.\cont. (
  action cont
    (\e.panic "unwrapped asynch call")
    \c.yield
)
