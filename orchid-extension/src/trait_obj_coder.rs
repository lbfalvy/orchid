pub struct TraitObject<T: ?Sized + Coding + 'static>(Box<T>, Arc<dyn Fn(&mut dyn Read) -> Self>);
impl<T: ?Sized + Coding + 'static> TraitObject<T> {
  fn inner_type_id(&self) -> ConstTypeId { self.0.as_ref().type_id() }
  fn get_decoder(&self) -> Arc<dyn Fn(&mut dyn Read) -> Self> { self.1.clone() }
}
pub trait AsTraitObject<T: ?Sized + Coding + 'static>: 'static {
  fn trait_box(self) -> Box<T>;
  fn into_trait_object(self) -> TraitObject<T>
  where Self: Sized + Coding {
    let decoder = Self::get_decoder(Self::into_trait_object);
    TraitObject(self.trait_box(), Arc::new(decoder))
  }
}

pub struct TraitObjectCoder<T: ?Sized + Coding + 'static> {
  entries: HashMap<u64, Box<dyn Fn(&mut dyn Read) -> TraitObject<T>>>,
}
impl<T: ?Sized + Coding + 'static> TraitObjectCoder<T> {
  pub fn add_type<U: AsTraitObject<T> + Coding>(&mut self, tid_hash: u64) {
    self.entries.entry(tid_hash).or_insert_with(|| Box::new(|b| U::decode(b).into_trait_object()));
  }
  pub fn add_obj(&mut self, tid_hash: u64, obj: &TraitObject<T>) {
    self.entries.entry(tid_hash).or_insert_with(|| {
      let decoder = obj.get_decoder();
      Box::new(move |b| decoder(b))
    });
  }
  pub fn encode<U: AsTraitObject<T> + Coding>(&mut self, data: U, out: &mut impl Write) {
    let tid = hash_tid(ConstTypeId::of::<U>());
    tid.encode(out);
    self.add_type::<U>(tid);
    data.encode(out);
  }
  pub fn encode_obj(&mut self, data: &TraitObject<T>, out: &mut impl Write) {
    let tid = hash_tid(data.inner_type_id());
    self.add_obj(tid, data);
    tid.encode(out);
    data.0.as_ref().encode(out);
  }
  pub fn decode(&mut self, src: &mut impl Read) -> TraitObject<T> {
    let tid = u64::decode(src);
    (self.entries.get(&tid).expect("Received object of unknown ConstTypeId"))(src)
  }
}