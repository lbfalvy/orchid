use std::io::{self, Error, Write};

pub fn cmd_prompt(prompt: &str) -> Result<(String, Vec<String>), Error> {
  print!("{}", prompt);
  io::stdout().flush()?;
  let mut cmdln = String::new();
  io::stdin().read_line(&mut cmdln)?;
  let mut segments = cmdln.split(' ');
  let cmd = if let Some(cmd) = segments.next() { cmd } else { "" };
  Ok((cmd.to_string(), segments.map(str::to_string).collect()))
}
