use crate::interpreted::ExprInst;

/// A thread-safe handle that can be used to send events of any type
pub trait MessagePort: Send + Clone + 'static {
  /// Send an event. Any type is accepted, handlers are dispatched by type ID
  fn send<T: Send + 'static>(&mut self, message: T);
}

pub trait Asynch {
  /// A thread-safe handle that can be used to push events into the dispatcher
  type Port: MessagePort;

  /// Register a function that will be called synchronously when an event of the
  /// accepted type is dispatched. Only one handler may be specified for each
  /// event type. The handler may choose to process the event autonomously, or
  /// return an Orchid thunk for the interpreter to execute.
  ///
  /// # Panics
  ///
  /// When the function is called with an argument type it was previously called
  /// with
  fn register<T: 'static>(
    &mut self,
    f: impl FnMut(Box<T>) -> Option<ExprInst> + 'static,
  );

  /// Return a handle that can be passed to worker threads and used to push
  /// events onto the dispatcher
  fn get_port(&self) -> Self::Port;
}
