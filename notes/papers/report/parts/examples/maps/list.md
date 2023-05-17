# List

In order to use lists as tuples, one needs to be able to access arbitrary elements by index. This is done by the new `list::get` function which returns an `option`. Since most lists in complex datastructures are of known length, this leads to a lot of unreachable branches. The marking and elimination of these called for the definition of `option::unwrap` and `std::panic`.