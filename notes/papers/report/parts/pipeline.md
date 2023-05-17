## The pipeline

The conversion of Orchid files into a collection of macro rules is a relatively complicated process that took several attempts to get right.

### Push vs pull logistics

The initial POC implementation of Orchid used pull logistics aka lazy evaluation everywhere. This meant that specially annotated units of computation would only be executed when other units referenced their result. This is a classic functional optimization, but its implementation in Rust had a couple drawbacks; First, lazy evaluation conflicts with most other optimizations, because it's impossible to assert the impact of a function call. Also - although this is probably a problem with my implementation - because the caching wrapper stores a trait object of Fn, every call to a stage is equivalent to a virtual function call which alone is sometimes an excessive penalty. Second, all values must live on the heap and have static lifetimes. Eventually nearly all fields referenced by the pipeline or its stages were wrapped in Rc.

Additionally, in a lot of cases lazy evaluation is undesirable. Most programmers other than the developers of Python would like to receive syntax errors in dead functions because statically identifiable errors are usually either typos that are trivial to fix or born out of a misconception on the programmer's part which is worth addressing in case it produces silent errors elsewhere. But errors are produced when the calculation of a value fails, so to produce errors all values about all functions msut be calculated.

To address these issues, the second iteration only uses pull logistics for the preparsing and file collection phase, and the only errors guaranteed to be produced by this stage are imports from missing files and syntax errors regarding the structure of the S-expressions.

### Stages

As of writing, the pipeline consists of three main stages; source loading, tree-building and name resolution. These break down into multiple substages.

All stages support various ways to introduce blind spots and precomputed values into their processing. This is used to load the standard library, prelude, and possibly externally defined intermediate stages of injected code.

#### Source loading

This stage encapsulates pull logistics. It collects all source files that should be included in the compilation in a hashmap keyed by their project-relative path. All subsequent operations are executed on every element of this map unconditionally.

The files and directory listings are obtained from an injected function for flexibility. File collection is motivated by a set of target paths, and injected paths can be ignored with a callback.

Parsing itself is outsourced to a Chumsky parser defined separately. This parser expects a list of operators for tokenization, but such a list is not available without knowledge of other files because glob imports don't provide information about the operators they define so much of the parsed data is invalid. What is known to be valid are

- the types of all lines
- line types `import` and `export`
- the pattern of `rule` lines
- the name of `constant` and `namespace` lines
- valid parts of the `exported` variant of lines
- valid parts of the body of `namespace` lines

This information is compiled into a very barebones module representation and returned alongside the loaded source code.

#### Tree building

This stage aims to collect all modules in a single tree. To achieve this, it re-parses each file with the set of operators collected from the datastructure built during preparsing. The glob imports in the resulting FileEntry lists are eliminated, and the names in the bodies of expressions and macro rules are prefixed with the module path in preparation for macro execution.

Operator collection can be advised about the exports of injected modules using a callback, and a prelude in the form of a list of line objects - in the shape emitted by the parser - can be injected before the contents of every module to define universally accessible names. Since these lines are processed for every file, it's generally best to just insert a single glob import from a module that defines everything. The interpreter inserts `import prelude::*`.

#### Import resolution

This stage aims to produce a tree ready for consumption by a macro executor or any other subsystem. It replaces every name originating from imported namespaces in every module with the original name.

Injection is supported with a function which takes a path and, if it's valid in the injected tree, returns its original value even if that's the path itself. This is used both to skip resolving names in the injected modules - which are expected to have already been processed using this step - and of course to find the origin of imports from the injected tree.

### Layered parsing

The most important export of the pipeline is the `parse_layer` function, which acts as a façade over the complex system described above. The environment in which user code runs is bootstrapped using repeated invocations of this function. It has the following options

1. targets that motivate file loading

    In the case of intermediate layers this can be a list of all included module names. The targets are only required to be valid, global import paths without a globstar.

2. a function that performs file and directory reads.
  
    This is normally set to a lambda that relays requests to `pipeline::file_loader`, but it can be replaced with another function if source code is to be loaded from an emulated file system, such as an in-memory tree or an online package repository.

3. the previous layer as an environment
4. a prelude to every file

    The interpreter sets this to `import prelude::*`. If the embedder defines its own prelude it's a good idea to append it.

#### The first layer

The other important exports of the pipeline are `ConstTree` and `from_const_tree`. These are used to define a base layer that exposes extern functions. `ConstTree` implements `Add` so distinct libraries of extern functions can be intuitively combined.