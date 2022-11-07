In Orchid, types and typeclasses aren't distinguished. Impls are the equivalent of typeclass
implementations. The syntax looks like this:

```orc
impl typeExpression [by name [over overriddenName, furtherOverriddenNames...]] via valueExpression
```

An impl can be considered a candidate for an auto if its typeExpression unifies with the auto's type
An impl candidate can be used to resolve an auto if
- typeExpression unifies with the auto's type
- it is not present in any other matching impl's override tree
- all other candidates are present in its override tree

### Impls for types

Impls for types are generally not a good idea as autos with types like Int can
often be used in dependent typing to represent eg. an index into a type-level conslist to be
deduced by the compiler, and impls take precedence over resolution by unification.

In Rust impls can be placed in one of two modules; the trait owner, and the type owner. In orchid
that is not the case, so two additional possibilities arise that Rust's orphan rules prevent.

## Foster impls

If it doesn't make sense for either of the participants to acknowledge the others, foster impls
can be created which don't own any of the participant symbols.

```orc
import GenericModule::Typeclass
import SpecificModule::(Type, function)

impl Typeclass Type by fosterTypeclassType via function
```

Foster impls can be placed in foster packages whose sole purpose is to glue packages together, or
they can be embedded in usercode.

## Arbiter impls

If multiple foster impls exist for a given package, or if you use a foster package but one of the
parents involved has added an impl in the mean time, ambiguities arise. To resolve these, arbiter
impls can be used to decide which impl's value will win.

``` orc
import BadModule::badImpl
import GoodModule::goodImpl
import GenericModule::Typeclass
import SpecificModule::Type

impl Typeclass Type by arbiterGoodModuleTypeclassType over goodImpl, badImpl via goodImpl
```

Notice that goodImpl appears both as a value and an impl name. Named impls are always also
exported as value substitution rules, specifically to account for situations where you want to use
them despite auto resolution. They can be referenced in arbiter rules, exception rules for more
general impls, auto-parameter overrides, and directly used as values in code.

The more common and less hacky use case for arbiter rules is when a very general rule from a
general package needs to be overridden by a more specific rule from a deep ancestor.

---

In all cases, these problems represent a concern gap or overlap and should be eventually resolved
by the authors of the original packages. The purpose of foster and arbiter rules is to not stall
the ecosystem on a trivial conflict of concepts and to make adding dependencies less risky.
It should still take some effort to maintain a large dependency list, but the risk of complete
blockage becomes a more manageable constant effort.