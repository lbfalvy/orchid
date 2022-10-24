# Steps of validating typed lambda

- Identify all expressions that describe the type of the same expression
- enqueue evaluation steps for each of them and put them in a unification group
- evaluation step refers to previous step, complete expression tree
  - unification **succeeds** if either
    - the trees are syntactically identical in any two steps between the targets
    - unification succeeds for all substeps:
      - try to find an ancestor step that provably produces the same value as any lambda in this
        step (for example, by syntactic equality)
        - if found, substitute it with the recursive normal form of the lambda
          - recursive normal form is `Apply(Y, \r.[body referencing r on point of recursion])`
      - find all `Apply(\x.##, ##)` nodes in the tree and execute them
  - unification **fails** if a member of the concrete tree differs (only outermost steps add to
    the concrete tree so it belongs to the group and not the resolution) or no substeps are found
    for a resolution step _(failure: unresolved higher kinded type)_
  - if neither of these conclusions is reached within a set number of steps, unification is
    **indeterminate** which is also a failure but suggests that the same value-level operations
    may be unifiable with better types.

The time complexity of this operation is O(h no) >= O(2^n). For this reason, a two-stage limit
is recommended: one for the recursion depth which is replicable and static, and another,
configurable, time-based limit enforced by a separate thread.

How does this interact with impls?
Idea: excluding value-universe code from type-universe execution.
Digression: Is it possible to recurse across universes?