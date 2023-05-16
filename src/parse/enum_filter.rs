/// Produces filter_mapping functions for enum types:
/// ```rs
/// enum_parser!(Foo::Bar | "Some error!") // Accepts Foo::Bar(T) into T
/// enum_parser!(Foo::Bar) // same as above but with the default error "Expected Foo::Bar"
/// enum_parser!(Foo >> Quz; Bar, Baz) // Parses  Foo::Bar(T) into Quz::Bar(T) and Foo::Baz(U) into Quz::Baz(U)
/// ```
#[macro_export]
macro_rules! enum_filter {
  ($p:path | $m:tt) => {
    {
      |l| {
        if let $p(x) = l { Ok(x) }
        else { Err($m) }
      }
    }
  };
  ($p:path >> $q:path; $i:ident | $m:tt) => {
    {
      use $p as srcpath;
      use $q as tgtpath;
      let base = enum_filter!(srcpath::$i | $m);
      move |l| base(l).map(tgtpath::$i)
    }
  };
  ($p:path >> $q:path; $i:ident) => {
    enum_filter!($p >> $q; $i | {concat!("Expected ", stringify!($i))})
  };
  ($p:path >> $q:path; $($i:ident),+ | $m:tt) => {
    {
      use $p as srcpath;
      use $q as tgtpath;
      |l| match l {
        $( srcpath::$i(x) => Ok(tgtpath::$i(x)), )+
        _ => Err($m)
      }
    }
  };
  ($p:path >> $q:path; $($i:ident),+) => {
    enum_filter!($p >> $q; $($i),+ | {
      concat!("Expected one of ", $(stringify!($i), " "),+)
    })
  };
  ($p:path) => {
    enum_filter!($p | {concat!("Expected ", stringify!($p))})
  };
}