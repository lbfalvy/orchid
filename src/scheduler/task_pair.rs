use crate::utils::translate::process;

use super::{Task, Nice, Priority, TaskState};

enum TaskPairState<T: Task, U: Task> {
  Empty,
  Left(T, U::Result),
  Right(T::Result, U),
  Both(T, U)
}

pub struct TaskPair<T: Task, U: Task> {
  l_nice: Nice,
  r_nice: Nice,
  state: TaskPairState<T, U>,
  tally: Priority,
}

impl<T: Task, U: Task> TaskPair<T, U> {
  pub fn new(l_nice: Nice, left: T, r_nice: Nice, right: U) -> Self {
    Self {
      l_nice, r_nice,
      tally: 0,
      state: TaskPairState::Both(left, right)
    }
  }
}

/// The state machine logic, abstracted from the subtask handling system
macro_rules! main_logic {
  ($self:ident, $task:ident, $task_runner:expr) => {{
    let TaskPair{ state, tally, l_nice, r_nice } = $self;
    let ret = process(state, |s| match s {
      TaskPairState::Empty => panic!("Generator completed and empty"),
      TaskPairState::Left(mut $task, r_res) => {
        match $task_runner {
          TaskState::Complete(r) => (TaskPairState::Empty, TaskState::Complete((r, r_res))),
          TaskState::Yield => (TaskPairState::Left($task, r_res), TaskState::Yield),
        }
      }
      TaskPairState::Right(l_res, mut $task) => {
        match $task_runner {
          TaskState::Complete(r) => (TaskPairState::Empty, TaskState::Complete((l_res, r))),
          TaskState::Yield => (TaskPairState::Right(l_res, $task), TaskState::Yield),
        }
      }
      TaskPairState::Both(l_task, r_task) => {
        let state = if 0 <= *tally {
          *tally -= *l_nice as Priority;
          let mut $task = l_task;
          match $task_runner {
            TaskState::Complete(r) => TaskPairState::Right(r, r_task),
            TaskState::Yield => TaskPairState::Both($task, r_task),
          }
        } else {
          *tally += *r_nice as Priority;
          let mut $task = r_task;
          match $task_runner {
            TaskState::Complete(r) => TaskPairState::Left(l_task, r),
            TaskState::Yield => TaskPairState::Both(l_task, $task),
          }
        };
        (state, TaskState::Yield)
      }
    });
    ret
  }};
}

impl<T: Task, U: Task> Task for TaskPair<T, U> {
  type Result = (T::Result, U::Result);

  fn run_n_times(&mut self, mut count: u64) -> TaskState<Self::Result> {
    loop {
      if count == 0 {return TaskState::Yield}
      match self.state {
        TaskPairState::Left(..) | TaskPairState::Right(..) => {
          return main_logic!(self, task, task.run_n_times(count));
        }
        _ => ()
      }
      if let r@TaskState::Complete(_) = self.run_once() {return r}
      count -= 1;
    }
  }

  fn run_once(&mut self) -> TaskState<Self::Result> {
    main_logic!(self, task, task.run_once())
  }
}
