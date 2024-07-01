The role and relations of the crates in this monorepo

# orcx

Reference runtime built using [`orchid-host`](#orchid-host)

# orchid-std

Standard library and reference extension built using [`orchid-extension`](#orchid-extension)

# orchid-host

Interpreter library with extension host. This is built to support embedding in arbitrary host applications for scripting, although it is not security hardened.

# orchid-extension

Extension development toolkit. It routes the requests to handlers in a nice object model, and manages resources like `ExprTicket`.

# orchid-base

Common items used by both [`orchid-host`](#orchid-host) and [`orchid-extension`](#orchid-extension). Most notably, both sides of the string interner are defined here because it has to be callable from other items defined in this crate through the same set of global functions in both environments.

# orchid-api

Definition of the extension API. This crate should not contain any logic, it should hold only serializable message structs, and their relations should represent how they fit into the protocol.

# orchid-api-derive

Derive macros for the traits in [`orchid-api-traits`](#orchid-api-traits) to make the definitions in [`orchid-api`](#orchid-api) more concise.

# orchid-api-traits

Traits with a semantic meaning with respect to the protocol elements defined in [`orchid-api`](#orchid-api):

- `Encode`, `Decode` and `Coding` define how types serialize.
- `Request` associates requests with their response types.
- `InHierarchy`, `Extends` and `UnderRoot` associate requests and notifications with the category hierarchy they belong to.