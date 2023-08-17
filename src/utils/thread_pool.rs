use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

/// A trait for a task dispatched on a [ThreadPool]. The task owns all relevant
/// data, is safe to pass between threads and is executed only once.
pub trait Task: Send + 'static {
  fn run(self);
}

impl<F: FnOnce() + Send + 'static> Task for F {
  fn run(self) {
    self()
  }
}

pub trait Query: Send + 'static {
  type Result: Send + 'static;

  fn run(self) -> Self::Result;
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
/// use orchidlang::ThreadPool;
///
/// let pool = ThreadPool::new(|s: String, _| println!("{}", s));
///
/// // spawns first thread
/// pool.submit("foo".to_string());
/// // probably spawns second thread
/// pool.submit("bar".to_string());
/// // either spawns third thread or reuses first
/// pool.submit("baz".to_string());
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
