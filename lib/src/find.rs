//! Provides an implementation of Future for locating a web_sys::Element by its
//! id.
//!
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};
use wasm_bindgen::UnwrapThrowExt;
use mogwai::utils::{timeout, window};


#[derive(Clone)]
pub struct Found<T> {
  pub found: T,
  pub elapsed: f64
}


pub struct FoundFuture<T> {
  op: Box<dyn Fn() -> Option<T>>,
  timeout: u32,
  poll_count: u64,
  start: f64,
}


impl<T> FoundFuture<T> {
  pub fn new<F>(timeout: u32, f:F) -> Self
  where
    F: Fn() -> Option<T> + 'static
  {
    FoundFuture {
      op: Box::new(f),
      timeout,
      poll_count: 0,
      start: 0.0
    }
  }

  pub fn run(&self) -> Option<T> {
    (self.op)()
  }
}


impl<T> Future for FoundFuture<T> {
  type Output = Option<Found<T>>;

  fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
    println!("polling");
    let now =
      window()
      .performance()
      .expect("no performance object")
      .now();

    let future = self.get_mut();

    // Do some timing upkeep
    if future.poll_count == 0 {
      future.start = now;
    }
    future.poll_count += 1;

    // Look for the thing
    let may_stuff:Option<T> = future.run();
    let elapsed = now - future.start;
    let elapsed_millis = elapsed.round() as u32;

    if may_stuff.is_none() && elapsed_millis <= future.timeout {
      // Set a timeout to wake this future on the next JS frame...
      let waker =
        Arc::new(Mutex::new(Some(
          ctx
            .waker()
            .clone()
        )));
      timeout(0, move || {
        let mut waker_var =
          waker
          .try_lock()
          .expect("could not acquire lock on ElementFuture waker");
        let waker:Waker =
          waker_var
          .take()
          .expect("could not unwrap stored waker on ElementFuture");
        waker.wake();

        // Don't automatically reschedule
        false
      });

      Poll::Pending
    } else if may_stuff.is_some() {
      let found = may_stuff.unwrap_throw();
      let now =
        window()
        .performance()
        .expect("no performance object")
        .now();

      Poll::Ready(Some(Found {
        elapsed: now - future.start,
        found
      }))
    } else {
      Poll::Ready(None)
    }
  }
}
