# The pipeline

The conversion of Orchid files into a collection of macro rules is a relatively complicated process. First, the source files are loaded and an initial parsing pass is executed. Because the set of supported operators influences the correct lexing of expressions, the output of this pass can't directly be used. The parts of each module that are known to be valid are

- the imports, because they don't use expressions at all
- the visibility and pattern of macro rule definitions, because it is required to separate distinct operators with spaces
- the visibility and name of constant definitions
- the name of submodules and these same elements in their bodies

This preparsed data is then used to locate all files in the solution, and to collect all operators visible to a certain file for a final parsing pass. It is necessary to refer to imported modules for a complete list of operators because glob imports don't offer any information about the set of names but still import all operators for the purpose of lexing.

## Push vs pull logistics

The initial POC implementation of Orchid used pull logistics aka lazy evaluation everywhere. This meant that specially annotated units of computation would only be executed when other units referenced their result. This is a classic functional optimization, but its implementation in Rust had a couple drawbacks; First, lazy evaluation conflicts with most other optimizations, because it's impossible to assert the impact of a function call. Also - although this is probably a problem with my implementation - because the caching wrapper stores a trait object of Fn, every call to a stage is equivalent to a virtual function call which alone is sometimes an excessive penalty. Second, all values must live on the heap and have static lifetimes. Eventually nearly all fields referenced by the pipeline or its stages were wrapped in Rc.

Additionally, in a lot of cases lazy evaluation is undesirable. Most programmers other than the developers of Python would like to receive syntax errors in dead functions because statically identifiable errors are usually either typos that are trivial to fix or born out of a misconception on the programmer's part which is worth addressing in case it produces silent errors elsewhere. But errors are produced when the calculation of a value fails, so to produce errors all values about all functions msut be calculated.

To address these issues, the second iteration only uses pull logistics for the preparsing and file collection phase, and the only errors guaranteed to be produced by this stage are imports from missing files and syntax errors regarding the structure of the S-expressions.