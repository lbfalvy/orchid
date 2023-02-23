{-# LANGUAGE FlexibleInstances #-}
{-# LANGUAGE MultiParamTypeClasses #-}
{-# LANGUAGE InstanceSigs #-}
{-# LANGUAGE BlockArguments #-}
import Prelude((>>=), Maybe( Just, Nothing ), return, fmap)
import Debug.Trace

-- 1
class Add l r o where
  add :: l -> r -> o
  (+) :: l -> r -> o
  (+) = add

-- 2
class Mappable c where
  map :: (i -> o) -> c i -> c o

-- 3
class Mappable c => Zippable c where
  zip :: (l -> r -> o) -> c l -> c r -> c o


-- 4
class Default t where
  def :: t

-- 5
instance (Zippable c, Add l r o)
  => Add (c l) (c r) (c o) where
  add :: (Zippable c, Add l r o) => c l -> c r -> c o
  add = zip add



-- 6
-- newtype List t = List (Maybe (t, List t))

-- instance Mappable List where
--   map :: (i -> o) -> List i -> List o
--   map f (List o) = List (fmap (\(h, t) -> (f h, map f t)) o)

-- instance Zippable List where
--   zip :: (l -> r -> o) -> List l -> List r -> List o
--   zip f (List l) (List r) = List do
--     (lh, lt) <- l
--     (rh, rt) <- r
--     return (f lh rh, zip f lt rt)

-- instance Add (List e) (List e) (List e) where
--   add (List l) (List r) = List case l of
--     Just (head, tail) -> Just (head, add tail r)
--     Nothing -> r

data List t = Cons t (List t) | End

instance Mappable List where
  map :: (i -> o) -> List i -> List o
  map _ End = End
  map f (Cons head tail) = Cons (f head) (map f tail)

instance Zippable List where
  zip :: (l -> r -> o) -> List l -> List r -> List o
  zip _ _ End = End
  zip _ End _ = End
  zip f (Cons lhead ltail) (Cons rhead rtail) =
    Cons (f lhead rhead) (zip f ltail rtail)

instance Add (List e) (List e) (List e) where
  add End r = r
  add (Cons head tail) r = Cons head (add tail r)

  