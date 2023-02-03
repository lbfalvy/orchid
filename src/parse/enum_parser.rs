/// Produces parsers for tokenized sequences of enum types:
/// ```rs
/// enum_parser!(Foo::Bar | "Some error!") // Parses Foo::Bar(T) into T
/// enum_parser!(Foo::Bar) // same as above but with the default error "Expected Foo::Bar"
/// enum_parser!(Foo >> Quz; Bar, Baz) // Parses  Foo::Bar(T) into Quz::Bar(T) and Foo::Baz(U) into Quz::Baz(U)
/// ```
#[macro_export]
macro_rules! enum_parser {
  ($p:path | $m:tt) => {
    {
      ::chumsky::prelude::filter_map(|s, l| {
        if let $p(x) = l { Ok(x) }
        else { Err(::chumsky::prelude::Simple::custom(s, $m))}
      })
    }
  };
  ($p:path >> $q:path; $i:ident) => {
    {
      use $p as srcpath;
      use $q as tgtpath;
      enum_parser!(srcpath::$i | (concat!("Expected ", stringify!($i)))).map(tgtpath::$i)
    } 
  };
  ($p:path >> $q:path; $($i:ident),+) => {
    {
      ::chumsky::prelude::choice((
        $( enum_parser!($p >> $q; $i) ),+
      ))
    }
  };
  ($p:path) => { enum_parser!($p | (concat!("Expected ", stringify!($p)))) };
}