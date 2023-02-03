use std::{ops::{Generator, GeneratorState}, pin::Pin};

use super::{Task, Nice, TaskState};

pub struct GeneratorTask<G: Generator<(), Yield = ()>> {
  nice: Nice,
  generator: Pin<Box<G>>
}

impl<G> GeneratorTask<G> where G: Generator<(), Yield = ()> {
  fn new(nice: Nice, generator: G) -> Self { Self {
    nice,
    generator: Box::pin(generator)
  } }
}

impl<G> Task for GeneratorTask<G>
where G: Generator<(), Yield = ()> {
  type Result = G::Return;

  fn run_once(&mut self) -> super::TaskState<Self::Result> {
    match self.generator.as_mut().resume(()) {
      GeneratorState::Yielded(()) => super::TaskState::Yield,
      GeneratorState::Complete(r) => super::TaskState::Complete(r)
    }
  }
}

impl<T> Task for Pin<Box<T>> where T: Generator<(), Yield = ()> {
  type Result = T::Return;

  fn run_once(&mut self) -> super::TaskState<Self::Result> {
    match self.as_mut().resume(()) {
      GeneratorState::Yielded(()) => TaskState::Yield,
      GeneratorState::Complete(r) => TaskState::Complete(r)
    }
  }
}

#[macro_export]
macro_rules! subtask {
  ($g:tt) => { {
    let task = $g;
    loop {
      match task.run_once() {
        TaskState::Yield => yield;
        TaskState::Complete(r) => break r;
      }
    }
  } };
}