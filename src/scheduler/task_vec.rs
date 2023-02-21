use std::{iter, mem};

use itertools::Itertools;

use super::{Task, Nice, TaskState};

const NORMALIZATION_THRESHOLD:Nice = Nice::MAX / 4;

struct TaskEntry<T: Task> {
  nice: Nice,
  position: usize,
  tally: Nice,
  task: T
}

struct TaskVec<T: Task> {
  results: Vec<Option<T::Result>>,
  task_heap: Vec<Option<TaskEntry<T>>>,
}

impl<T: Task> TaskVec<T> {
  pub fn new(tasks: Vec<(Nice, T)>) -> Self {
    let mut results = Vec::with_capacity(tasks.len());
    results.resize_with(tasks.len(), || None);
    let task_heap = tasks.into_iter().enumerate()
      .map(|(position, (nice, task))| Some(TaskEntry{ nice, task, position, tally: 1 }))
      .collect_vec();
    Self { results, task_heap }
  }
  
  fn entry(&self, i: usize) -> Option<&TaskEntry<T>> {
    if self.task_heap.len() <= i {None}
    else {self.task_heap[i].as_ref()}
  }
  fn entry_mut(&mut self, i: usize) -> Option<&mut TaskEntry<T>> {
    if self.task_heap.len() <= i {None}
    else {self.task_heap[i].as_mut()}
  }
  /// Returns the tally of the given record. Empty records always sink to the bottom
  fn tally(&self, i: usize) -> Nice {
    self.task_heap[i].as_ref().map(|e| e.tally).unwrap_or(Nice::MAX)
  }
  fn swap(&mut self, a: usize, b: usize) {
    self.task_heap.swap(a, b);
  }
  fn iter_mut(&mut self) -> impl Iterator<Item = &mut TaskEntry<T>> {
    self.task_heap.iter_mut().filter_map(|e| e.as_mut())
  }

  fn normalize(&mut self) {
    let shrink_count = self.task_heap.iter().rev().take_while(|e| e.is_none()).count();
    let new_len = self.task_heap.len() - shrink_count;
    self.task_heap.splice(new_len.., iter::empty());
    let head = self.entry_mut(0);
    let offset = if let Some(e) = head {
      let offset = e.tally - 1;
      if offset < NORMALIZATION_THRESHOLD {return}
      e.tally = 1;
      offset
    } else {return};
    for entry in self.iter_mut() { entry.tally -= offset }
  }

  fn sink(&mut self, i: usize) {
    let lchi = 2*i + 1;
    let rchi = 2*i + 2;
    let t = self.tally(i);
    let lcht = if let Some(e) = self.entry(lchi) {e.tally} else {
      if self.tally(rchi) < t {
        self.swap(rchi, i);
        self.sink(rchi);
      }
      return
    };
    let rcht = if let Some(e) = self.entry(rchi) {e.tally} else {
      if self.tally(lchi) < t {
        self.swap(lchi, i);
        self.sink(lchi);
      }
      return
    };
    let mchi = {
      if rcht < t && rcht < lcht {rchi}
      else if lcht < t && lcht < rcht {lchi}
      else {return}
    };
    self.swap(i, mchi);
    self.sink(mchi);
  }

  fn take_results(&mut self) -> Vec<T::Result> {
    let mut swap = Vec::new();
    mem::swap(&mut self.results, &mut swap);
    return swap.into_iter().collect::<Option<_>>()
      .expect("Results not full but the heap is empty");
  }

  fn one_left(&mut self) -> bool {
    self.entry(0).is_some() && self.entry(1).is_none() && self.entry(2).is_none()
  }
}

impl<T: Task> Task for TaskVec<T> {
  type Result = Vec<T::Result>;

  fn run_n_times(&mut self, mut count: u64) -> TaskState<Self::Result> {
    loop {
      if count == 0 {return TaskState::Yield}
      if self.one_left() {
        let head = &mut self.task_heap[0];
        let head_entry = head.as_mut().expect("one_left faulty");
        return match head_entry.task.run_n_times(count) {
          TaskState::Yield => TaskState::Yield,
          TaskState::Complete(r) => {
            self.results[head_entry.position] = Some(r);
            *head = None;
            return TaskState::Complete(self.take_results());
          }
        }
      } else if let r@TaskState::Complete(_) = self.run_once() {return r}
      count -= 1;
    }
  }

  fn run_once(&mut self) -> super::TaskState<Self::Result> {
    self.normalize();
    let head = &mut self.task_heap[0];
    let head_entry = head.as_mut().expect("All completed, cannot run further");
    head_entry.tally += head_entry.nice;
    match head_entry.task.run_once() {
      TaskState::Complete(r) => {
        self.results[head_entry.position] = Some(r);
        *head = None;
        self.sink(0);
        if self.entry(0).is_some() { return TaskState::Yield }
        TaskState::Complete(self.take_results())
      }
      TaskState::Yield => {
        head_entry.tally += head_entry.nice;
        self.sink(0);
        TaskState::Yield
      }
    }
  }
}