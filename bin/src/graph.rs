use mogwai::gizmo::*;
use std::collections::HashMap;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::SvgsvgElement;

use super::bench_runner::Benchmark;


fn lang_color(lang: Option<&String>) -> &str {
    let lang: Option<&str> = lang.as_ref().map(|s| s.as_str());
    match lang {
        Some("rust") => "darkorange",
        Some("javascript") => "gold",
        Some("elm") => "darkturquoise",
        Some("clojurescript") => "mediumorchid",
        Some("haskell") => "mediumpurple",
        _ => "grey",
    }
}


struct GraphableBenchmark {
    benches: Vec<Benchmark>,
}


impl GraphableBenchmark {
    fn new() -> Self {
        GraphableBenchmark { benches: vec![] }
    }

    fn total(&self) -> f64 {
        if self.benches.len() == 0 {
            return 0.0;
        }

        let sum = self
            .benches
            .iter()
            .fold(0.0, |sum, bench| sum + bench.todos_deleted.1);
        sum / self.benches.len() as f64
    }

    fn average(&self) -> Benchmark {
        let mut benchmark = Benchmark::new();
        for bench in self.benches.iter() {
            benchmark.name = bench.name.clone();
            benchmark.failed_message = bench.failed_message.clone();
            benchmark.language = bench.language.clone();
            benchmark.load.0 += bench.load.0;
            benchmark.load.1 += bench.load.1;
            benchmark.await_todo.0 += bench.await_todo.0;
            benchmark.await_todo.1 += bench.await_todo.1;
            benchmark.todos_creation.0 += bench.todos_creation.0;
            benchmark.todos_creation.1 += bench.todos_creation.1;
            benchmark.todos_completed.0 += bench.todos_completed.0;
            benchmark.todos_completed.1 += bench.todos_completed.1;
            benchmark.todos_deleted.0 += bench.todos_deleted.0;
            benchmark.todos_deleted.1 += bench.todos_deleted.1;
        }
        let len = self.benches.len() as f64;
        benchmark.load.0 /= len;
        benchmark.load.1 /= len;
        benchmark.await_todo.0 /= len;
        benchmark.await_todo.1 /= len;
        benchmark.todos_creation.0 /= len;
        benchmark.todos_creation.1 /= len;
        benchmark.todos_completed.0 /= len;
        benchmark.todos_completed.1 /= len;
        benchmark.todos_deleted.0 /= len;
        benchmark.todos_deleted.1 /= len;
        benchmark
    }

    fn name(&self) -> String {
        self.benches
            .first()
            .map(|b| b.name.clone())
            .unwrap_or("none".into())
    }

    fn has_error(&self) -> bool {
        for bench in self.benches.iter() {
            if bench.failed_message.is_some() {
                return true;
            }
        }
        false
    }
}


fn graph_entries(benchmarks: &Vec<GraphableBenchmark>) -> Vec<Gizmo<Element>> {
    let mut max_total = 0.0;
    let mut max_name_width = 0.0;
    let font_size = 12.0;
    for bench in benchmarks.into_iter() {
        let text_width = bench.name().len() as f32 * font_size;
        max_name_width = f32::max(text_width, max_name_width);
        let total = bench.total();
        max_total = f64::max(max_total, total);
    }

    let padding = 8.0;
    let lane_height = font_size + padding;
    let bar_height = lane_height - 2.0;
    let local_bar_y = (lane_height - bar_height) / 2.0;
    let graph_start = max_name_width * 0.7;
    let max_bar_width = 960.0 - graph_start;
    let mut next_y = font_size;
    let mut tags = vec![];

    for graphable_bench in benchmarks.into_iter() {
        let bench = graphable_bench.average();
        log::trace!("{}", bench.name);
        let text_width = bench.name.len() as f32 * font_size;
        let text_x = graph_start - text_width;
        let text_y = next_y + (lane_height / 2.0) + (font_size / 2.0);
        log::trace!("  text_width: {}, text_x: {}", text_width, text_x);
        log::trace!("  next_y: {}", next_y);
        let text = Gizmo::element_ns("text", "http://www.w3.org/2000/svg")
            .attribute("font-family", "monospace")
            .attribute("font-size", "12")
            .text(&bench.name)
            .attribute("x", &format!("{}", 0))
            .attribute("y", &format!("{}", text_y))
            .upcast::<Element>();

        let underline = Gizmo::element_ns("path", "http://www.w3.org/2000/svg")
            .attribute(
                "d",
                &format!(
                    "M 0 {} H {}",
                    (next_y + lane_height - 2.0).floor() as u32,
                    graph_start + 2.0
                ),
            )
            .attribute("stroke", lang_color(bench.language.as_ref()));

        let total = bench.total();
        let total_text_string = format!("{}ms", total.round() as u32);
        let total_text = Gizmo::element_ns("text", "http://www.w3.org/2000/svg")
            .class("framework-text")
            .attribute("x", &format!("{}", graph_start + font_size))
            .attribute("y", &format!("{}", text_y))
            .text(&total_text_string);

        let to_x_and_width = |x0: f32, x1: f32| -> (f32, f32) {
            let x_percent = x0 / max_total as f32;
            let width_percent = (x1 - x0) / max_total as f32;
            let x = x_percent * max_bar_width;
            let width = width_percent * max_bar_width;
            (graph_start + x, width)
        };

        if let Some(fail_msg) = bench.failed_message.as_ref() {
            let text = Gizmo::element_ns("text", "http://www.w3.org/2000/svg")
                .attribute("font-family", "monospace")
                .attribute("font-size", "12")
                .attribute(
                    "x",
                    &format!(
                        "{}",
                        graph_start + font_size + (total_text_string.len() as f32 * font_size)
                    ),
                )
                .attribute("y", &format!("{}", text_y))
                .text(&format!("Failed: {}", fail_msg));
            tags.push(text);
        } else {
            let (_, rect_width) = to_x_and_width(0.0, bench.total() as f32);
            let rect = Gizmo::element_ns("rect", "http://www.w3.org/2000/svg")
                .attribute("x", &format!("{}", graph_start))
                .attribute("y", &format!("{}", next_y + local_bar_y))
                .attribute("width", &format!("{}", rect_width))
                .attribute("height", &format!("{}", bar_height))
                .attribute("fill", lang_color(bench.language.as_ref()))
                .attribute("opacity", "0.4")
                .with(
                    Gizmo::element_ns("title", "http://www.w3.org/2000/svg")
                        .text(&format!("total bench time - {}ms", total.round() as u32)),
                );
            tags.push(rect);

            for (event_name, event_start, event_end) in bench.event_deltas() {
                let (x, width) = to_x_and_width(event_start as f32, event_end as f32);
                let rect = Gizmo::element_ns("rect", "http://www.w3.org/2000/svg")
                    .attribute("x", &format!("{}", x))
                    .attribute("y", &format!("{}", next_y + local_bar_y))
                    .attribute("width", &format!("{}", width))
                    .attribute("height", &format!("{}", bar_height))
                    .attribute("fill", lang_color(bench.language.as_ref()))
                    .attribute("stroke", "white")
                    .attribute("stroke-width", "1px")
                    .with(
                        Gizmo::element_ns("title", "http://www.w3.org/2000/svg").text(&format!(
                            "{} - {}ms",
                            event_name,
                            (event_end - event_start).round() as u32
                        )),
                    );

                tags.push(rect);
            }
        }

        tags.push(text);
        tags.push(underline);
        tags.push(total_text);

        next_y = next_y + lane_height;
    }

    tags
}


fn process_benchmarks(benchmarks: &Vec<Benchmark>) -> Vec<GraphableBenchmark> {
    let mut bench_map: HashMap<String, GraphableBenchmark> = HashMap::new();
    for benchmark in benchmarks.iter() {
        let entry = bench_map
            .entry(benchmark.name.clone())
            .or_insert(GraphableBenchmark::new());
        entry.benches.push(benchmark.clone());
    }

    bench_map.into_iter().map(|(_, v)| v).collect()
}

pub fn graph_benchmarks(benchmarks: &Vec<Benchmark>) -> Gizmo<SvgsvgElement> {
    let mut graph = Gizmo::element_ns("svg", "http://www.w3.org/2000/svg")
        .attribute("width", "960")
        .attribute("height", "540")
        .attribute("viewBox", "0 0 960 540")
        .class("embed-responsive-item");

    let mut benchmarks = process_benchmarks(benchmarks);
    benchmarks.sort_by(|bencha, benchb| {
        let a = bencha.total().round() as u32;
        let b = benchb.total().round() as u32;
        let time_ord = a.cmp(&b);
        if bencha.has_error() {
            std::cmp::Ordering::Greater
        } else if benchb.has_error() {
            std::cmp::Ordering::Less
        } else {
            time_ord
        }
    });

    for entry in graph_entries(&benchmarks).into_iter() {
        graph = graph.with(entry);
    }

    graph.downcast().map_err(|_| ()).unwrap_throw()
}
