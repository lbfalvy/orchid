## Async conversion

convert host to async non-send

demonstrate operation with existing lex-hello example

consider converting extension's SysCtx to a typed context bag

align fn atom and macros on both sides with new design. No global state.

## alternate extension mechanism

The Macro extension needs to be in the same compilation unit as the interpreter because the interpreter needs to proactively access its datastructures (in particular, it needs to generate MacTree from TokTree)

Ideally, it should reuse `orchid-extension` for message routing and decoding.

`orchid-host` accepts extensions as `impl ExtensionPort`

## Preprocessor extension

Must figure out how preprocessor can both be a System and referenced in the interpreter

Must actually write macro system as recorded in note

At this point swappable preprocessors aren't a target because interaction with module system sounds complicated

Check if any of this needs interpreter, if so, start with that