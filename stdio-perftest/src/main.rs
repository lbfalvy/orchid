use std::env::{self, args};
use std::io::{stdin, BufRead, BufReader, Read, Write};
use std::process::{self, Child};
use std::time::SystemTime;

fn main() {
  let is_child = env::args().any(|arg| arg == "child");
  if is_child {
    loop {
      let mut input = String::new();
      stdin().read_line(&mut input).unwrap();
      if input == "ping\n" {
        println!("pong");
      } else if input.is_empty() {
        process::exit(0);
      } else {
        panic!("Unrecognized input {input:?}");
      }
    }
  } else {
    let steps = 10_000;
    let mut child = process::Command::new(args().next().unwrap())
      .arg("child")
      .stdin(process::Stdio::piped())
      .stdout(process::Stdio::piped())
      .spawn()
      .unwrap();
    let mut bufr = BufReader::new(child.stdout.take().unwrap());
    let mut child_stdin = child.stdin.take().unwrap();
    let time = SystemTime::now();
    for _ in 0..steps {
      writeln!(child_stdin, "ping");
      let mut buf = String::new();
      bufr.read_line(&mut buf).unwrap();
      if buf != "pong\n" {
        panic!("Unrecognized output {buf:?}")
      }
    }
    child.kill();
    let elapsed = time.elapsed().unwrap();
    let avg = elapsed / steps;
    println!("A roundtrip takes {avg:?}, {}ms on average", (avg.as_nanos() as f64) / 1_000_000f64);
  }
}
