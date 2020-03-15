use wasm_bindgen::UnwrapThrowExt;
use web_sys::{SvgsvgElement, SvgTextContentElement};
use mogwai::gizmo::*;

use super::bench_runner::Benchmark;


fn graph_entries(benchmarks: &Vec<(String, Benchmark)>) -> Vec<Gizmo<Element>> {
  let mut texts = vec![];
  let mut max_total = 0.0;
  let mut max_name_width = 0.0;
  let font_size = 12.0;
  for (name, bench) in benchmarks.iter() {
    let text =
      Gizmo::element_ns("text", "http://www.w3.org/2000/svg")
      .attribute("font-family", "monospace")
      .attribute("font-size", "12")
      .text(name)
      .downcast::<SvgTextContentElement>()
      .map_err(|_| ())
      .expect("could not create svg text element");
    let text_width = name.len() as f32 * font_size;
    log::trace!("text_width: {} {}", name, text_width);
    max_name_width = f32::max(text_width, max_name_width);
    let total = bench.total();
    texts.push((text, text_width, name, total));
    max_total = f64::max(max_total, total);
  }

  let padding = 8.0;
  let lane_height = font_size + padding;
  let bar_height = lane_height - 2.0;
  let local_bar_y = (lane_height - bar_height)/2.0;
  let graph_start = max_name_width;
  let max_bar_width = 960.0 - graph_start;
  log::trace!("max_name_width: {}", max_name_width);
  log::trace!("graph_start: {}", graph_start);

  let mut next_y = font_size;
  let mut tags = vec![];

  for (text, text_width, name, total) in texts.into_iter() {
    log::trace!("{}", name);
    let text_x = graph_start - text_width;
    let text_y = next_y + (lane_height/2.0) + (font_size/2.0);
    log::trace!("  text_width: {}, text_x: {}", text_width, text_x);
    log::trace!("  next_y: {}", next_y);
    let text =
      text
      .attribute("x", &format!("{}", 0))
      .attribute("y", &format!("{}", text_y))
      .upcast::<Element>();

    let total_text =
      Gizmo::element_ns("text", "http://www.w3.org/2000/svg")
      .class("framework-text")
      .attribute("x", &format!("{}", graph_start))
      .attribute("y", &format!("{}", text_y))
      .text(&format!("{}ms", total.round() as u32));

    let rect_width = max_bar_width as f64 * (total / max_total);
    let rect =
      Gizmo::element_ns("rect", "http://www.w3.org/2000/svg")
      .attribute("x", &format!("{}", graph_start))
      .attribute("y", &format!("{}", next_y + local_bar_y))
      .attribute("width", &format!("{}", rect_width))
      .attribute("height", &format!("{}", bar_height))
      .attribute("fill", "orange");

    tags.push(rect);
    tags.push(text);
    tags.push(total_text);

    next_y = next_y + lane_height;
  }

  tags
}

pub fn graph_benchmarks(benchmarks: &Vec<(String, Benchmark)>) -> Gizmo<SvgsvgElement> {
  let mut graph =
    Gizmo::element_ns("svg", "http://www.w3.org/2000/svg")
    .attribute("width", "960")
    .attribute("height", "540")
    .attribute("viewBox", "0 0 960 540")
    .class("embed-responsive-item");
  for entry in graph_entries(benchmarks).into_iter() {
    graph = graph.with(entry);
  }

  graph
    .downcast()
    .map_err(|_| ())
    .unwrap_throw()
}
