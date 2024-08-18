# Communication Architecture

- **Communication medium** 1-1 duplex ordered reliable and a request/reply notify protocol defined on top (`orchid-base` mostly)

- **Protocol definition** plain objects that define nothing beside serialization/deserialization and English descriptions of invariants (`orchid-api`)

- **Active objects** smart objects that represent resources and communicate commands and queries through the protocol, sorted into 3 categories for asymmetric communication: common/extension/host. Some ext/host logic is defined in `orchid-base` to ensure that other behaviour within it can rely on certain global functionality. ext/host also manage their respective connection state (`orchid-base`, `orchid-extension`, `orchid-host`)

- **Application** (client, server) binaries that use active objects to implement actual application behaviour (`orcx`, `orchid-std`)

