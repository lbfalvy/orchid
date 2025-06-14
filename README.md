<h1 align="center">
  <img src="icon.svg" alt="logo" height="60px">
  Orchid
</h1>

An experimental lazy, pure functional programming language designed to be embeddable in a Rust application for scripting.

## Usage

The standalone interpreter can be built as the binary target from this package. The language tutorial and standard library documentation is at [lbfalvy.github.io/orchid-reference/](https://lbfalvy.github.io/orchid-reference/). Embedder guide and Rust API documentation are coming soon.

## Design

The execution model is lambda calculus, with call by name and copy tracking to avoid repeating steps. This leads to the minimal number of necessary reduction steps.

To make the syntax more intuitive, completely hygienic macros can be used which are applied to expressions after all imports are resolved and all tokens are namespaced both in the macro and in the referencing expression.

Namespaces are inspired by Rust modules and ES6. Every file and directory is implicitly a public module. Files can `export` names of constants or namespaces, all names in a substitution rule, or explicitly export some names. Names are implicitly created when they're referenced. `import` syntax is similar to Rust except with `(` parentheses `)` and no semicolons.

## Try it out

The project uses the nighly rust toolchain. Go to one of the folders within `examples` and run

```sh
cargo run --release
```

you can try modifying the examples, but error reporting for the time being is pretty terrible.

## The name

Orchids and mangrove trees form complex ecosystems; The flowers persuade the tree to grow in different ways than it normally would to provide better support for their vines, and kill fungi and other pests. The metaphor is about vines growing on a tree and moving branches into more optimal positions for other vines.

## Contribution

All contributions are welcome. For the time being, use the issue tracker to discuss ideas.
