//! Provides an implementation of Future for locating a web_sys::Element by its
//! id.
//!
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};
use wasm_bindgen::UnwrapThrowExt;
use web_sys::Element;
use mogwai::utils::{document, timeout, window};


#[derive(Clone)]
pub struct FoundElement {
  pub element: Element,
  pub elapsed: u32
}


pub struct ElementFuture {
  id: String,
  timeout: u32,
  poll_count: u64,
  start: f64,
}


impl ElementFuture {
  pub fn new(id: String, timeout: u32) -> Self {
    ElementFuture {
      id,
      timeout,
      poll_count: 0,
      start: 0.0
    }
  }
}


impl Future for ElementFuture {
  type Output = Option<FoundElement>;

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

    // Look for the element
    let may_element:Option<Element> =
      document()
      .get_element_by_id(&future.id);

    let elapsed = now - future.start;
    let elapsed_millis = elapsed.round() as u32;

    if may_element.is_none() && elapsed_millis <= future.timeout {
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
    } else if may_element.is_some() {
      let element = may_element.unwrap_throw();
      let now =
        window()
        .performance()
        .expect("no performance object")
        .now();

      Poll::Ready(Some(FoundElement {
        elapsed: (now - future.start).round() as u32,
        element
      }))
    } else {
      Poll::Ready(None)
    }
  }
}
