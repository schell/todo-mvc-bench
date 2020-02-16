pub mod find;
pub mod framework_card;

use find::{FoundFuture, Found};


pub async fn wait_for<T, F>(
  millis: u32,
  f:F
) -> Option<Found<T>>
where
  F: Fn() -> Option<T> + 'static
{
  FoundFuture::new(millis, f).await
}
