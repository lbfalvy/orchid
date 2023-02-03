# Purpose

Type expressions are trees. Any single branch could terminate the solver and any branch may be nonterminating, therefore all of them must be run concurrently. Thread-based concurrency isn't an option because a compiler must be perfectly deterministic. It is also beneficial to have fine-grained control over the relative priority of different tasks.