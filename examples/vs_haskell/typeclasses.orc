






-- 1
define Add $L $R $O
as $L -> $R -> $O

$left:2... + $right:1... =1000=> add ($left...) ($right...)

-- 2
define Mappable $C:type -> type
as @I. @O. (I -> O) -> $C I -> $C O

-- 3
define Zippable $C:type -> type
as @:Mappable $C.
  @L. @R. @O. (L -> R -> O) -> $C L -> $C R -> $C O

-- 4
define Default $T:type as $T


-- 5
impl
  @C:Type -> Type. @L. @R. @O.
  @:(Zippable C). @:(Add L R O).
  Add (C L) (C R) (C O)
by elementwiseAdd
via zip add

-- 6
define List $E as Y \r. Option t[ $E, r ]

impl Mappable List
via \f.\list. categorise (
  (Y \repeat. \opt. match opt {
    Some t[head, tail] => 
      Some t[f head, repeat tail];
    None => None;
  }) (generalise list)
)

impl Zippable List 
via \f.\l.\r. categorise (
  Y \repeat.\lopt.\ropt. do {
    bind t[lhead, ltail] <- lopt;
    bind t[rhead, rtail] <- ropt;
    t[f lhead rhead, repeat ltail rtail]
  }
) (generalise l) (generalise r)

impl @T. Add (List T) (List T) (List T)
by concatListAdd over elementwiseAdd
via \l.\r.categorise Y \repeat.\l. (
  match l (
    Some t[head, tail] =>
      Some t[head, repeat tail];
    None => (generalise r)
  )
) (generalise l)

