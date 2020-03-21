pub mod find;
pub mod async_event;

use find::{FoundFuture, Found};


pub async fn wait_for<T, F>(
  millis: u32,
  f:F
) -> Result<Found<T>, f64>
where
  F: Fn() -> Option<T> + 'static
{
  FoundFuture::new(millis, f).await
}


pub async fn wait(millis: u32) -> f64 {
  let future = wait_for(millis, || { None as Option<Found<()>>});
  match future.await {
    Ok(Found{elapsed,..}) => elapsed,
    Err(elapsed) => elapsed
  }
}
