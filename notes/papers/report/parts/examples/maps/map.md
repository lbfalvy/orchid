# Map

A map implemented using a list of 2-length lists each containing a key and a corresponding value. Although `list` defines a `pair` for internal use, a binary `list` was chosen to test the performance of the interpreter.

While using a Church-pair instead of a list to store individual entries could multiply the performance of this map, a greater improvement can be achieved by using some sort of tree structure. This implementation is meant for very small maps such as those representing a typical struct.

## cover vs erase

In a list map like this one, most operations are O(n), except insertion which has an O(1) variant - appending a new frame with the new value without checking if one already exists. This is not generally a good idea, but in some extreme situations the time it saves can be very valuable.

