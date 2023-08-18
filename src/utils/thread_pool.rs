//! A thread pool for executing tasks in parallel, spawning threads as workload
//! increases and terminating them as tasks finish. This is not terribly
//! efficient, its main design goal is to parallelize blocking I/O calls.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

/// A trait for a task dispatched on a [ThreadPool]. The task owns all relevant
/// data, is safe to pass between threads and is executed only once.
pub trait Task: Send + 'static {
  /// Execute the task. At a minimum, this involves signaling some other thread,
  /// otherwise the task has no effect.
  fn run(self);
}

impl<F: FnOnce() + Send + 'static> Task for F {
  fn run(self) {
    self()
  }
}

/// An async unit of work that produces some result, see [Task]. This can be
/// wrapped in a generic reporter to create a task.
pub trait Query: Send + 'static {
  /// The value produced by the query
  type Result: Send + 'static;

  /// Execute the query, producing some value which can then be sent to another
  /// thread
  fn run(self) -> Self::Result;

  /// Associate the query with a reporter expressed in a plain function.
  /// Note that because every lambda has a distinct type and every thread pool
  /// runs exactly one type of task, this can appear only once in the code for
  /// a given thread pool. It is practical in a narrow set of cases, most of the
  /// time however you are better off defining an explicit reporter.
  fn then<F: FnOnce(Self::Result) + Send + 'static>(
    self,
    callback: F,
  ) -> QueryTask<Self, F>
  where
    Self: Sized,
  {
    QueryTask { query: self, callback }
  }
}
impl<F: FnOnce() -> R + Send + 'static, R: Send + 'static> Query for F {
  type Result = R;

  fn run(self) -> Self::Result {
    self()
  }
}

/// A reporter that calls a statically known function with the result of a
/// query. Constructed with [Query::then]
pub struct QueryTask<Q: Query, F: FnOnce(Q::Result) + Send + 'static> {
  query: Q,
  callback: F,
}
impl<Q: Query, F: FnOnce(Q::Result) + Send + 'static> Task for QueryTask<Q, F> {
  fn run(self) {
    (self.callback)(self.query.run())
  }
}

enum Message<T: Task> {
  Stop,
  Task(T),
}

struct ThreadPoolData<T: Task> {
  rdv_point: Mutex<Option<SyncSender<Message<T>>>>,
  stopping: AtomicBool,
}

/// A thread pool to execute blocking I/O operations in parallel.
/// This thread pool is pretty inefficient for CPU-bound operations because it
/// spawns an unbounded number of concurrent threads and destroys them eagerly.
/// It is assumed that the tasks at hand are substnatially but not incomparably
/// more expensive than spawning a new thread.
///
/// If multiple threads finish their tasks, one waiting thread is kept, the
/// rest exit. If all threads are busy, new threads are spawned when tasks
/// arrive. To get rid of the last waiting thread, drop the thread pool.
///
/// ```
/// use orchidlang::thread_pool::{Task, ThreadPool};
///
/// struct MyTask(&'static str);
/// impl Task for MyTask {
///   fn run(self) {
///     println!("{}", self.0)
///   }
/// }
///
/// let pool = ThreadPool::new();
///
/// // spawns first thread
/// pool.submit(MyTask("foo"));
/// // probably spawns second thread
/// pool.submit(MyTask("bar"));
/// // either spawns third thread or reuses first
/// pool.submit(MyTask("baz"));
/// ```
pub struct ThreadPool<T: Task> {
  data: Arc<ThreadPoolData<T>>,
}
impl<T: Task> ThreadPool<T> {
  /// Create a new thread pool. This just initializes the threadsafe
  /// datastructures used to synchronize tasks and doesn't spawn any threads.
  /// The first submission spawns the first thread.
  pub fn new() -> Self {
    Self {
      data: Arc::new(ThreadPoolData {
        rdv_point: Mutex::new(None),
        stopping: AtomicBool::new(false),
      }),
    }
  }

  /// Submit a task to the thread pool. This tries to send the task to the
  /// waiting thread, or spawn a new one. If a thread is done with its task
  /// and finds that it another thread is already waiting, it exits.
  pub fn submit(&self, task: T) {
    let mut standby = self.data.rdv_point.lock().unwrap();
    if let Some(port) = standby.take() {
      (port.try_send(Message::Task(task))).expect(
        "This channel cannot be disconnected unless the receiver crashes
        between registering the sender and blocking for receive, and it cannot
        be full because it's taken before insertion",
      );
    } else {
      drop(standby);
      let data = self.data.clone();
      // worker thread created if all current ones are busy
      spawn(move || {
        let mut cur_task = task;
        loop {
          // Handle the task
          cur_task.run();
          // Apply for a new task if no other thread is doing so already
          let mut standby_spot = data.rdv_point.lock().unwrap();
          if standby_spot.is_some() {
            return; // exit if we would be the second in line
          }
          let (sender, receiver) = sync_channel(1);
          *standby_spot = Some(sender);
          drop(standby_spot);
          if data.stopping.load(Ordering::SeqCst) {
            return; // exit if the pool was dropped before we applied
          }
          // Wait for the next event on the pool
          let msg = (receiver.recv()).expect("We are holding a reference");
          match msg {
            // repeat with next task
            Message::Task(task) => cur_task = task,
            // exit if the pool is dropped
            Message::Stop => return,
          }
        }
      });
    }
  }
}

impl<T: Task> Default for ThreadPool<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Task> Drop for ThreadPool<T> {
  // Ensure all threads exit properly
  fn drop(&mut self) {
    self.data.stopping.store(true, Ordering::SeqCst);
    let mut rdv_point = self.data.rdv_point.lock().unwrap();
    if let Some(pending) = rdv_point.take() {
      pending
        .try_send(Message::Stop)
        .expect("The channel is always removed before push")
    }
  }
}
