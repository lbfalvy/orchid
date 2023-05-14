# Impl

Impl is used to implement typeclasses. Impl is a distinct [[02-parsing#Files|line type]] that has the following form:
```
impl = "impl" target_type ["by" impl_name ["over" alternative*]] "via" value
target_type = clause*
impl_name = name
alternative = ns_name
value = clause*
```

Impls provide fallbacks for binding resolution. If the target type contains any @ bindings at the top level, they are also applied to the value, to avoid repetition. The list of alternatives contains references to other impls which the author of this impl is aware of and deems more general or for another reason inferior. Alternatives can never form a cycle.

## Matching rules

When a [[02-given|@]] binding is not resolvable using rules 1 and 2, impls are used to find a value. Each impl's target type may contain other bindings, so resolution proceeds similarly to a breadth-first Prolog solver.

An impl is considered an acceptable **candidate** for a binding if its type unifies with goal, with its bindings resolved in the context where the original binding is defined. This means that these indirect bindings are also first resolved using **assignable** enclosing bindings before impls would be enumerated.

An impl is considered a **match** if it is a **candidate**, and all other candidates are reachable from it by walking the alternative tree (even if the intermediate steps are not candidates). If there is no match, 

## Overrides

In Rust impls can be placed in one of two modules; the trait owner, and the type owner. Orchid is more forgiving than that which means that mistakes in external packages can temporarily be fixed in user code, but it also means that inconsistency is possible and needs to be addressed. Two additional possibilities arise that Rust's orphan rules prevent; foster impls and arbiter impls.

### Foster impls

If it doesn't make sense for either of the participants to acknowledge the others, foster impls can be created which don't own any of the participant symbols.

```orc
import GenericModule::Typeclass
import SpecificModule::(Type, function)

impl Typeclass Type by fosterTypeclassType via function
```

Foster impls can be placed in foster packages whose sole purpose is to glue packages together, or they can be embedded in usercode.

### Arbiter impls

If multiple foster impls exist for a given package, or if a foster impl is provided by some collection but one of the parents added an impl in the mean time, ambiguities arise. To resolve these, arbiter impls can be used to decide which value will win.

``` orc
import BadModule::badImpl
import GoodModule::goodImpl
import GenericModule::Typeclass
import SpecificModule::Type

impl Typeclass Type by arbiterGoodModuleTypeclassType over goodImpl, badImpl via goodImpl
```

Notice that goodImpl appears both as a value and an impl name. Named impls are always also exported as constants, specifically to account for situations where you want to use them despite auto resolution. They can be referenced in arbiter rules, exception rules for more general impls, and directly used as values in code.

The more common and less hacky use case for arbiter rules is when a very general rule from a general package needs to be overridden by a more specific rule from a deep ancestor.

---

In all cases, these problems represent a concern gap or overlap and should be eventually resolved by the authors of the original packages. The purpose of foster and arbiter rules is to not stall the ecosystem on a trivial conflict of concepts and to make adding dependencies less risky. It should still take some effort to maintain a large dependency list, but the risk of complete blockage becomes a more manageable constant effort.