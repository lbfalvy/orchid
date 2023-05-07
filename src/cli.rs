use std::{fmt::Display, io::{stdin, BufRead, stdout, Write}};

pub fn prompt<T: Display, E: Display>(
  prompt: &str,
  default: T,
  mut try_cast: impl FnMut(String) -> Result<T, E>
) -> T {
  loop {
    print!("{prompt} ({default}): ");
    stdout().lock().flush().unwrap();
    let mut input = String::with_capacity(100);
    stdin().lock().read_line(&mut input).unwrap();
    if input.len() == 0 {return default}
    match try_cast(input) {
      Ok(t) => return t,
      Err(e) => println!("Error: {e}")
    }
  }
}