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
  // Entry point
  ($input:expr, $($src:ident)::* => $tgt:ty {
    $($branches:tt)*
  } $({
    $($extra:tt)*
  })?) => {
    match_mapping!(@BRANCH_MUNCH
      (($input) ($($src)*) ($tgt) ($($($extra)*)?))
      ()
      $($branches)* ,
    )
    // note: we're adding a comma to the input so the optional trailing comma becomes
    // an optional second comma which is easier to match
  };
  // ======== Process match branches
  // Can't generate branches individually so gather them into a collection and render them here
  (@BRANCHES_DONE ( ($input:expr) $src:tt ($tgt:ty) ($($extra:tt)*) )
    $( ( $variant:ident $($pat:tt)*) )*
  ) => {
    {
      match $input {
        $(
          match_mapping!(@PAT ($src $variant) $($pat)*) =>
            match_mapping!(@VAL (< $tgt >:: $variant) $($pat)*),
        )*
        $($extra)*
      }
    }
  };
  // End with optional second comma
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $(,)?) => {
    match_mapping!(@BRANCHES_DONE $ext $($branches)* )
  };
  // Unit variant
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $variant:ident , $($tail:tt)*) => {
    match_mapping!(@BRANCH_MUNCH $ext ( $($branches)* ($variant) ) $($tail)*)
  };
  // Variant mapped to same shape pair
  (@BRANCH_MUNCH $ext:tt ( $($branches:tt)* ) $variant:ident $pat:tt , $($tail:tt)*) => {
    match_mapping!(@BRANCH_MUNCH $ext
      ( $($branches)* ($variant $pat) )
    $($tail)*)
  };
  (@PAT (($($prefix:tt)*) $variant:ident)) => { $($prefix ::)* $variant };
  (@PAT $prefix:tt ( $($fields:tt)* )) => {
    match_mapping!(@PAT_MUNCH (() $prefix) () $($fields)* ,)
  };
  (@PAT $prefix:tt { $($fields:tt)* }) => {
    match_mapping!(@PAT_MUNCH ({} $prefix) () $($fields)* ,)
  };
  (@PAT_MUNCH (() (($($prefix:ident)*) $variant:ident)) ($($names:ident)*)) => {
    $($prefix)::* :: $variant ( $($names),* )
  };
  (@PAT_MUNCH ({} (($($prefix:ident)*) $variant:ident)) ($($names:ident)*)) => {
    $($prefix)::* :: $variant { $($names),* }
  };
  (@PAT_MUNCH $ctx:tt $names:tt $(,)? ) => { match_mapping!($ctx $names) };
  (@PAT_MUNCH $ctx:tt ($($names:ident)*) * $name:ident , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH $ctx ($($names)* $name) $($tail)*)
  };
  (@PAT_MUNCH $ctx:tt ($($names:ident)*) $name:ident => $value:expr , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH $ctx ($($names)* $name) $($tail)*)
  };
  (@PAT_MUNCH $ctx:tt ($($names:ident)*) $name:ident () $value:expr , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH $ctx ($($names)* $name) $($tail)*)
  };
  (@PAT_MUNCH $ctx:tt ($($names:ident)*) $name:ident . $($tail:tt)*) => { 
    match_mapping!(@PAT_DOT_MUNCH $ctx ($($names)* $name) $($tail)*)
  };
  (@PAT_DOT_MUNCH $ctx:tt $names:tt , $($tail:tt)*) => {
    match_mapping!(@PAT_MUNCH $ctx $names $($tail)*)
  };
  (@PAT_DOT_MUNCH $ctx:tt $names:tt $_:tt $($tail:tt)*) => {
    match_mapping!(@PAT_DOT_MUNCH $ctx $names $($tail)*)
  };
  (@VAL ($($prefix:tt)*)) => { $($prefix)* };
  (@VAL $prefix:tt ( $($fields:tt)* )) => {
    match_mapping!(@VAL_MUNCH (() $prefix) () $($fields)* , )
  };
  (@VAL $prefix:tt { $($fields:tt)* }) => {
    match_mapping!(@VAL_MUNCH ({} $prefix) () $($fields)* , )
  };
  (@VAL_MUNCH $ctx:tt ($($prefix:tt)*) * $name:ident , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH $ctx ($($prefix)* ($name (* $name)) ) $($tail)*)
  };
  (@VAL_MUNCH $ctx:tt ($($prefix:tt)*) $name:ident => $value:expr , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH $ctx ($($prefix)* ($name ($value)) ) $($tail)*)
  };
  (@VAL_MUNCH $ctx:tt ($($prefix:tt)*) $name:ident () $value:expr , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH $ctx ($($prefix)* ($name ($value($name))) ) $($tail)*)
  };
  (@VAL_MUNCH $ctx:tt $fields:tt $name:ident . $member:tt $($tail:tt)*) => {
    match_mapping!(@VAL_DOT_MUNCH $ctx $fields $name ($name . $member ) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ctx:tt ($($fields:tt)*) $name:ident $current:tt , $($tail:tt)*) => {
    match_mapping!(@VAL_MUNCH $ctx ($($fields)* ($name $current)) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ctx:tt $fields:tt $name:ident ($($current:tt)*) $tt:tt $($tail:tt)*) => {
    match_mapping!(@VAL_DOT_MUNCH $ctx $fields $name ($($current)* $tt) $($tail)*)
  };
  (@VAL_DOT_MUNCH $ctx:tt ($($fields:tt)*) $name:ident $current:tt) => {
    match_mapping!(@VAL_MUNCH $ptyp ($($fields)* ($name $current)))
  };
  (@VAL_MUNCH $ctx:tt $fields:tt , ) => { match_mapping!(@VAL_MUNCH $ctx $fields) };
  (@VAL_MUNCH (() ($($prefix:tt)*)) ($( ( $name:ident $($value:tt)* ) )*) ) => {
    $($prefix)* ( $( $($value)* ),* )
  };
  (@VAL_MUNCH ({} ($($prefix:tt)*)) ($( ( $name:ident $($value:tt)* ) )*) ) => {
    $($prefix)* { $( $name : $($value)* ),* }
  };
}