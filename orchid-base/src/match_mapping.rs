/// A shorthand for mapping over enums with identical structure. Used for converting between
/// owned enums and the corresponding API enums that only differ in the type of their
/// fields.
/// 
/// The basic form is
/// ```ignore
/// match_mapping!(self, ThisType => OtherType {
///   EmptyVariant,
///   TupleVariant(foo => intern(foo), bar.clone()),
///   StructVariant{ a.to_api(), b => b.}
/// })
/// ```
#[macro_export]
macro_rules! match_mapping {
  ($input:expr, $src:ty => $tgt:ty {
    $($branches:tt)*
  }) => {
    match_mapping!(@BRANCH_MUNCH (($input) ($src) ($tgt)) () $($branches)* ,)
  };
  (@BRANCHES_DONE ( ($input:expr) ($src:ty) ($tgt:ty) )
    $( ( $variant:ident $($pat:tt)*) )*
  ) => {
    {
      use $src as Foo;
      match $input {
        $(
          match_mapping!(@PAT (Foo :: $variant) $($pat)*) =>
            match_mapping!(@VAL (< $tgt >:: $variant) $($pat)*),
        )*
      }
    }
  };
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $(,)?) => {
    match_mapping!(@BRANCHES_DONE $ext $($branches)* )
  };
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $variant:ident , $($tail:tt)*) => {
    match_mapping!(@BRANCH_MUNCH $ext ( $($branches)* ($variant) ) $($tail)*)
  };
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $variant:ident $pat:tt , $($tail:tt)*) => {
    match_mapping!(@BRANCH_MUNCH $ext
      ( $($branches)* ($variant $pat) )
    $($tail)*)
  };
  (@PAT ($($prefix:tt)*) ( $($fields:tt)* )) => {
    $($prefix)* ( match_mapping!(@PAT_MUNCH () $($fields)*) )
  };
  (@PAT ($($prefix:tt)*) { $($fields:tt)* }) => {
    $($prefix)* { match_mapping!(@PAT_MUNCH () $($fields)*) }
  };
  (@PAT ($($path:tt)*)) => { $($path)* };
  (@PAT_MUNCH ($($names:ident)*) $name:ident => $value:expr) => { $($names ,)* $name };
  (@PAT_MUNCH ($($names:ident)*) $name:ident => $value:expr , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH ($($names)* $name) $($tail)*)
  };
  (@PAT_MUNCH ($($names:ident)*) $name:ident . $($tail:tt)*) => { 
    match_mapping!(@PAT_DOT_MUNCH ($($names)* $name) $($tail)*)
  };
  (@PAT_MUNCH ($($names:ident)*)) => { $($names),* };
  (@PAT_DOT_MUNCH $names:tt , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH $names $($tail)*)
  };
  (@PAT_DOT_MUNCH $names:tt $_:tt $($tail:tt)*) => {
    match_mapping!(@PAT_DOT_MUNCH $names $($tail)*)
  };
  (@PAT_DOT_MUNCH ($($names:tt)*)) => { $($names),* };
  (@VAL ($($prefix:tt)*)) => { $($prefix)* };
  (@VAL ($($prefix:tt)*) ( $($fields:tt)* )) => {
    $($prefix)* ( match_mapping!(@VAL_MUNCH () () $($fields)* ) )
  };
  (@VAL ($($prefix:tt)*) { $($fields:tt)* }) => {
    $($prefix)* { match_mapping!(@VAL_MUNCH {} () $($fields)* ) }
  };
  (@VAL_MUNCH () ($($prefix:tt)*) $name:ident => $value:expr) => { $($prefix)* $value };
  (@VAL_MUNCH () ($($prefix:tt)*) $name:ident => $value:expr , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH () ($($prefix)* $value, ) $($tail)*)
  };
  (@VAL_MUNCH {} ($($prefix:tt)*) $name:ident => $value:expr) => { $($prefix)* $name: $value };
  (@VAL_MUNCH {} ($($prefix:tt)*) $name:ident => $value:expr , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH {} ($($prefix)* $name: $value, ) $($tail)*)
  };
  (@VAL_MUNCH () ($($prefix:tt)*) $name:ident . $member:tt $($tail:tt)*) => {
    match_mapping!(@VAL_DOT_MUNCH () ($($prefix)* $name . $member ) $($tail)*)
  };
  (@VAL_MUNCH {} ($($prefix:tt)*) $name:ident . $member:tt $($tail:tt)*) => {
    match_mapping!(@VAL_DOT_MUNCH {} ($($prefix)* $name: $name . $member) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ptyp:tt ($($prefix:tt)*) , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH $ptyp ($($prefix)* ,) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ptyp:tt ($($prefix:tt)*) $tt:tt $($tail:tt)*) => {
    match_mapping!(@VAL_DOT_MUNCH $ptyp ($($prefix)* $tt) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ptyp:tt ($($prefix:tt)*)) => { $($prefix)* };
  (@VAL_MUNCH $_ptyp:tt ($($prefix:tt)*)) => { $($prefix)* };
}