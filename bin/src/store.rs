use wasm_bindgen::JsValue;
use web_sys::Storage;
use serde_json;
use mogwai::utils;

use super::bench_runner::Benchmark;

const KEY: &str = "todo-mvc-bench";

pub fn write_items(items: &Vec<Benchmark>) -> Result<(), JsValue> {
  let str_value =
    serde_json::to_string(items)
    .expect("Could not serialize benchmarks");
  utils::window()
    .local_storage()?
    .into_iter()
    .for_each(|storage:Storage| {
      storage
        .set_item(KEY, &str_value)
        .expect("could not store serialized bencmarks");
    });
  Ok(())
}

pub fn read_benchmarks() -> Result<Vec<Benchmark>, JsValue> {
  let storage =
    utils::window()
    .local_storage()?
    .expect("Could not get local storage");

  let may_item_str: Option<String> =
    storage
    .get_item(KEY)
    .expect("Error using storage get_item");

  let items =
    may_item_str
    .map(|json_str:String| {
      let items:Vec<Benchmark> =
        serde_json::from_str(&json_str)
        .unwrap_or(vec![]);
      items
    })
    .unwrap_or(vec![]);

  Ok(items)
}
