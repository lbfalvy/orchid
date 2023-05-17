## Interpreter

The Orchid interpreter exposes one main function called `run`. This function takes an expression to reduce and the symbol table returned by the pipeline and processed by the macro repository. It's also possible to specify a reduction step limit to make sure the function returns in a timely manner.

### Interfacing with an embedder

An embedding application essentially interacts with Orchid by way of queries, that is, it invokes the interpreter with a prepared function call. The Orchid code then replies with a return value, which the embedder can either read directly or use as a component in subsequent questions, and so the conversation develops. All communication is initiated, regulated and the conclusions executed entirely by the embedder.

Although external functions are exposed to Orchid and they can be called at any time (within a computation), they are expected to be pure and any calls to them may be elided by the optimizer if it can deduce the return value from precedent or circumstances.

One common way to use a query API is to define a single query that is conceptually equivalent to "What would you like to do?" and a set of valid answers which each incorporate some way to pass data through to the next (identical) query. HTTP does this, historically client state was preserved in cookies and pre-filled form inputs, later with client-side Javascript and LocalStorage. 

Orchid offers a way to do this using the `Handler` trait and the `run_handler` function which is the interpreter's second important export. Essentially, this trait offers a way to combine functions that match and process various types implmeenting `Atomic`. This allows embedders to specify an API where external functions return special, inert `Atomic` instances corresponding to environmental actions the code can take, each of which also carries the continuation of the logic. This is a variation of continuation passing style, a common way of encoding effects in pure languages. It is inspired by algebraic effects