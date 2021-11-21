//! Provides an implementation of Future for locating a web_sys::Element by its
//! id.
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use mogwai::{
    futures::stream::{self, Stream, StreamExt},
    prelude::broadcast,
};

#[derive(Clone)]
pub struct Found<T> {
    pub found: T,
    pub elapsed_seconds: f64,
}

pub async fn wait_for<T: 'static>(
    timeout_seconds: f64,
    f: impl FnMut() -> Option<T> + 'static,
) -> Result<Found<T>, f64> {
    let start = mogwai::utils::window()
        .performance()
        .expect("no performance object")
        .now();

    let f = Arc::new(Mutex::new(f));

    loop {
        let (tx_done, mut rx_done) = futures::channel::oneshot::channel();
        let (tx_tick, mut rx_tick) = futures::channel::oneshot::channel();
        let f = f.clone();
        mogwai::time::set_immediate(move || {
            let mut f_lock = f.lock().unwrap();
            if let Some(t) = f_lock() {
                let _ = tx_done.send(t).ok().unwrap();
            } else {
                tx_tick.send(()).unwrap();
            }
        });

        futures::select_biased! {
            res = rx_done => {
                let now = mogwai::utils::window()
                    .performance()
                    .expect("no performance object")
                    .now();
                let elapsed_seconds = (now - start) / 1000.0;

                return res.map(|t| Found {
                    found: t,
                    elapsed_seconds,
                })
                    .map_err(|_| elapsed_seconds);
            },
            res = rx_tick => {
                let now = mogwai::utils::window()
                    .performance()
                    .expect("no performance object")
                    .now();
                let elapsed_seconds = (now - start) / 1000.0;

                if let Err(e) = res {
                    log::error!("error finding: {}", e);
                    return Err(elapsed_seconds);
                }

                if elapsed_seconds >= timeout_seconds {
                    return Err(elapsed_seconds);
                }
            }
        }
    }
}

/// Wait while the given polling function returns true.
pub async fn wait_while(
    timeout_seconds: f64,
    mut f: impl FnMut() -> bool + 'static,
) -> Result<Found<()>, f64> {
    wait_for(timeout_seconds, move || if f() { None } else { Some(()) }).await
}

pub async fn wait_until_next_for<T>(
    timeout_seconds: f64,
    stream: impl Stream<Item = T> + Unpin,
) -> Result<Found<T>, f64> {
    let start = mogwai::utils::window()
        .performance()
        .expect("no performance object")
        .now();

    let mut stream = stream.fuse();
    let mut timeout = mogwai::time::wait_approx(timeout_seconds * 1000.0).fuse();

    mogwai::futures::select! {
        may_t = stream.next() => {
            let now = mogwai::utils::window()
                .performance()
                .expect("no performance object")
                .now();

            let elapsed_seconds = (now - start) / 1000.0;

            if let Some(t) = may_t {
                Ok(Found {
                    found: t,
                    elapsed_seconds
                })
            } else {
                Err(elapsed_seconds)
            }

        }
        elapsed_millis = timeout => {
            Err(elapsed_millis / 1000.0)
        }
    }
}
