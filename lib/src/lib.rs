pub mod find;
pub mod framework_card;

use find::{ElementFuture, FoundElement};


pub async fn wait_for_element(id: &str, millis: u32) -> Option<FoundElement> {
  let id:String = id.into();
  let may_future = ElementFuture::new(id, millis);
  may_future.await
}
