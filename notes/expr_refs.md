# Reference hierarchy of the host

Reference loops are resource leaks. There are two primary ways to avoid reference loops; a strict hierarchy between types is the easiest. The expression tree uses a less obvious hierarchy where expressions are only able to reference expressions that are older than them.

- Trees reference Constants
- Constants reference their constituent Expressions
- Expressions reference Atoms
- During evaluation, Constants replace their unbound names with Constants
  - There is a reference cycle here, but it always goes through a Constant.
    > **todo** A potential fix may be to update all Constants to point to a dummy value before freeing Trees
- Atoms reference the Systems that implement them
- Atoms may reference Expressions that are not younger than them
  - This link is managed by the System but tied to Atom and not System lifecycle
  - Atoms can technically be applied to themselves, but it's a copying apply so it probably isn't a risk factor
- Systems reference the Extension that contains them
- Extensions reference the Port that connects them
  - The Extension signals the remote peer to disconnect on drop
  - The port is also referenced in a loose receiver thread, which always eventually tries to find the Extension or polls for ingress so it always eventually exits after the Extension's drop handler is called
