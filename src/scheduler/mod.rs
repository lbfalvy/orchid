mod generator_task;
mod task_pair;
mod task_vec;

pub type Nice = u16;
pub type Priority = i32;

pub enum TaskState<R> {
  Yield,
  Complete(R)
}

pub trait Task {
  type Result;
  
  fn run_once(&mut self) -> TaskState<Self::Result>;

  fn run_n_times(&mut self, count: u64) -> TaskState<Self::Result> {
    for _ in 0..count {
      if let r@TaskState::Complete(_) = self.run_once() {
        return r
      }
    }
    return TaskState::Yield
  }

  fn run_to_completion(&mut self) -> Self::Result {
    loop { if let TaskState::Complete(r) = self.run_once() {return r} }
  }

  fn boxed<'a>(self) -> TaskBox<'a, Self::Result> where Self: 'a + Sized { Box::new(self) }
}

pub type TaskBox<'a, T> = Box<dyn Task<Result = T> + 'a>;

impl<'a, R> Task for TaskBox<'a, R> {
  type Result = R;

  fn run_once(&mut self) -> TaskState<Self::Result> { self.as_mut().run_once() }
  fn run_n_times(&mut self, count: u64) -> TaskState<Self::Result> {
    self.as_mut().run_n_times(count)
  }

  fn run_to_completion(&mut self) -> Self::Result {
    self.as_mut().run_to_completion()
  }
}