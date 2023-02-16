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

impl<T: Task, U: Task> Task for TaskPair<T, U> {
  type Result = (T::Result, U::Result);

  fn run_once(&mut self) -> TaskState<Self::Result> {
    let TaskPair{ state, tally, l_nice, r_nice } = self;
    let ret = process(state, |s| match s {
      TaskPairState::Empty => panic!("Generator completed and empty"),
      TaskPairState::Left(mut l_task, r_res) => {
        match l_task.run_once() {
          TaskState::Complete(r) => (TaskPairState::Empty, TaskState::Complete((r, r_res))),
          TaskState::Yield => (TaskPairState::Left(l_task, r_res), TaskState::Yield),
        }
      }
      TaskPairState::Right(l_res, mut r_task) => {
        match r_task.run_once() {
          TaskState::Complete(r) => (TaskPairState::Empty, TaskState::Complete((l_res, r))),
          TaskState::Yield => (TaskPairState::Right(l_res, r_task), TaskState::Yield),
        }
      }
      TaskPairState::Both(mut l_task, mut r_task) => {
        let state = if 0 <= *tally {
          *tally -= *l_nice as Priority;
          match l_task.run_once() {
            TaskState::Complete(r) => TaskPairState::Right(r, r_task),
            TaskState::Yield => TaskPairState::Both(l_task, r_task),
          }
        } else {
          *tally += *r_nice as Priority;
          match r_task.run_once() {
            TaskState::Complete(r) => TaskPairState::Left(l_task, r),
            TaskState::Yield => TaskPairState::Both(l_task, r_task),
          }
        };
        (state, TaskState::Yield)
      }
    });
    ret
  }
}
