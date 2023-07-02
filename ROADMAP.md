# IO

All IO is event-based via callbacks.
Driven streams such as stdin expose single-fire events for the results of functions such as "read until terminator" or "read N bytes".
Network IO exposes repeated events such as "connect", "message", etc.