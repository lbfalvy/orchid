#[macro_export]
macro_rules! clone {
  ($($n:ident),+; $body:expr) => (
    {
      $( let $n = $n.clone(); )+
      $body
    }
  );
  ($($n:ident),+) => {
    $( let $n = $n.clone(); )+
  }
}
