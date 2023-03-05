pub trait Visit<T> {
  type Return;
  fn visit(&self, target: T) -> Return; 
}

pub trait ImpureVisit<T> {
  type Shard;
  type Return;
  fn impure_visit(&self, target: T) -> (Shard, Return);
  fn merge(&mut self, s: Shard);
}

pub struct OverlayVisitor<VBase, VOver>(VBase, VOver);

impl<VBase, VOver, T, R> Visitor<T> for OverlayVisitor<VBase, VOver>
where VBase: Visitor<T, Return = Option<R>>, VOver: Visitor<T, Return = Option<R>> {
  
}