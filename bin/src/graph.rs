use mogwai::prelude::*;
use std::{collections::HashMap, convert::TryFrom};
use wasm_bindgen::UnwrapThrowExt;
use web_sys::{SvgElement, SvgsvgElement};

use super::bench_runner::{Benchmark, BenchmarkStep};

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

#[derive(Debug)]
struct BenchmarkDatum {
    name: String,
    points: Vec<(f64, f64)>,
}

impl BenchmarkDatum {
    fn average_span(&self) -> (f64, f64) {
        let (s, e) = self
            .points
            .iter()
            .fold((0.0, 0.0), |(start, end), (s, e)| (start + s, end + e));
        (s / self.points.len() as f64, e / self.points.len() as f64)
    }
}

impl TryFrom<&BenchmarkStep> for BenchmarkDatum {
    type Error = String;
    fn try_from(step: &BenchmarkStep) -> Result<Self, String> {
        let end = step.end.ok_or(format!("{} has no end", step.name))?;

        Ok(BenchmarkDatum {
            name: step.name.clone(),
            points: vec![(step.start, end)],
        })
    }
}

#[derive(Debug)]
struct GraphableBenchmark {
    name: String,
    language: Option<String>,
    error: Option<String>,
    data: Vec<BenchmarkDatum>,
}

impl GraphableBenchmark {
    fn merge_data(&mut self, data: Vec<BenchmarkDatum>) {
        let mut hm: HashMap<String, BenchmarkDatum> = data
            .into_iter()
            .map(|datum| (datum.name.clone(), datum))
            .collect();
        for datum in self.data.iter_mut() {
            if let Some(other_datum) = hm.remove(&datum.name) {
                datum.points.extend(other_datum.points);
            }
        }
        let leftover: Vec<BenchmarkDatum> = hm.into_iter().map(|(_k, v)| v).collect();
        self.data.extend(leftover);
    }

    fn max_bench_len(&self) -> f64 {
        self.data.iter().fold(0.0, |max_len, datum| {
            f64::max(max_len, datum.average_span().1)
        })
    }
}

fn graph_entries(benchmarks: &Vec<GraphableBenchmark>) -> Vec<View<SvgElement>> {
    let mut max_total = 0.0;
    let mut max_name_width = 0.0;
    let font_size = 12.0;
    for bench in benchmarks.into_iter() {
        let text_width = bench.name.len() as f32 * font_size;
        max_name_width = f32::max(text_width, max_name_width);
        let total = bench.max_bench_len();
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

    for gbench in benchmarks.into_iter() {
        log::trace!("{:#?}", gbench);
        let text_width = gbench.name.len() as f32 * font_size;
        let text_x = graph_start - text_width;
        let text_y = next_y + (lane_height / 2.0) + (font_size / 2.0);
        log::trace!("  text_width: {}, text_x: {}", text_width, text_x);
        log::trace!("  next_y: {}", next_y);
        let text = (View::element_ns("text", "http://www.w3.org/2000/svg") as View<SvgElement>)
            .attribute("font-family", "monospace")
            .attribute("font-size", "12")
            .with(View::from(&gbench.name) as View<Text>)
            .attribute("x", &format!("{}", 0))
            .attribute("y", &format!("{}", text_y));

        let total = gbench.max_bench_len();
        let total_text_string = format!("{}ms", total.round() as u32);
        let total_text = (View::element_ns("text", "http://www.w3.org/2000/svg")
            as View<SvgElement>)
            .attribute("class", "framework-text")
            .attribute("x", &format!("{}", font_size))
            .attribute("y", &format!("{}", text_y + lane_height))
            .with(View::from(&total_text_string) as View<Text>);

        let to_x_and_width = |x0: f32, x1: f32| -> (f32, f32) {
            let x_percent = x0 / max_total as f32;
            let width_percent = (x1 - x0) / max_total as f32;
            let x = x_percent * max_bar_width;
            let width = width_percent * max_bar_width;
            (x, f32::max(width, 1.0))
        };

        next_y += lane_height;

        if let Some(fail_msg) = gbench.error.as_ref() {
            let text = (View::element_ns("text", "http://www.w3.org/2000/svg") as View<SvgElement>)
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
                .with(View::from(&format!("Failed: {}", fail_msg)) as View<Text>);
            tags.push(text);
        } else {
            let (_, rect_width) = to_x_and_width(0.0, total as f32);
            let rect = (View::element_ns("rect", "http://www.w3.org/2000/svg") as View<SvgElement>)
                .attribute("x", &format!("{}", 0))
                .attribute("y", &format!("{}", next_y + local_bar_y))
                .attribute("width", &format!("{}", rect_width))
                .attribute("height", &format!("{}", bar_height))
                .attribute("fill", lang_color(gbench.language.as_ref()))
                .attribute("opacity", "0.4")
                .with(
                    (View::element_ns("title", "http://www.w3.org/2000/svg") as View<SvgElement>)
                        .with(
                            View::from(&format!("total bench time - {}ms", total.round() as u32))
                                as View<Text>,
                        ),
                );
            tags.push(rect);

            for datum in gbench.data.iter() {
                assert!(
                    datum.points.len() > 0,
                    format!("no points in datum '{}'", datum.name)
                );

                let (min, max) = datum.points.iter().fold(
                    (f64::INFINITY, f64::NEG_INFINITY),
                    |(n, x), (start, end)| (f64::min(n, *start), f64::max(x, *end)),
                );
                let (start, end) = datum.average_span();
                let (x, width) = to_x_and_width(start as f32, end as f32);
                log::trace!(
                    "{:#?} min:{} max:{} x:{} width:{}",
                    datum,
                    min,
                    max,
                    x,
                    width
                );
                let event_bar = (View::element_ns("rect", "http://www.w3.org/2000/svg")
                    as View<SvgElement>)
                    .attribute("x", &format!("{}", x))
                    .attribute("y", &format!("{}", next_y + 1.0))
                    .attribute("width", &format!("{}", width))
                    .attribute("height", &format!("{}", bar_height))
                    .attribute("rx", &format!("{}", bar_height / 2.0))
                    .attribute("fill", lang_color(gbench.language.as_ref()))
                    .attribute("stroke", "white")
                    .attribute("opacity", "0.6")
                    .attribute("style", "cursor: pointer;")
                    .with(
                        (View::element_ns("title", "http://www.w3.org/2000/svg")
                            as View<SvgElement>)
                            .with(View::from(&format!(
                                "{} took {}ms ({} to {})",
                                datum.name,
                                (end - start).round() as u32,
                                start.round() as u32,
                                end.round() as u32
                            )) as View<Text>),
                    );
                tags.push(event_bar);
                //let (x, width) = to_x_and_width(event_start as f32, event_end as f32);
                //let rect = Gizmo::element_ns("rect", "http://www.w3.org/2000/svg")
                //    .attribute("x", &format!("{}", x))
                //    .attribute("y", &format!("{}", next_y + local_bar_y))
                //    .attribute("width", &format!("{}", width))
                //    .attribute("height", &format!("{}", bar_height))
                //    .attribute("fill", lang_color(bench.language.as_ref()))
                //    .attribute("stroke", "white")
                //    .attribute("stroke-width", "1px")
                //    .with(
                //        Gizmo::element_ns("title", "http://www.w3.org/2000/svg").text(&format!(
                //            "{} - {}ms",
                //            event_name,
                //            (event_end - event_start).round() as u32
                //        )),
                //    );

                //tags.push(rect);
            }
            next_y += bar_height;
        }

        tags.push(text);
        tags.push(total_text);
    }

    tags
}

fn process_benchmark_data(steps: &Vec<BenchmarkStep>) -> Vec<BenchmarkDatum> {
    steps.iter().flat_map(BenchmarkDatum::try_from).collect()
}

fn process_benchmarks(benchmarks: &Vec<Benchmark>) -> Vec<GraphableBenchmark> {
    let mut bench_map: HashMap<String, GraphableBenchmark> = HashMap::new();
    for benchmark in benchmarks.iter() {
        log::trace!("{:#?}", benchmark);
        let entry = bench_map
            .entry(benchmark.name.clone())
            .or_insert(GraphableBenchmark {
                name: benchmark.name.clone(),
                language: benchmark.language.clone(),
                error: benchmark.failed_message.clone(),
                data: vec![],
            });
        let data = process_benchmark_data(&benchmark.steps);
        log::trace!("{:#?}", data);
        entry.merge_data(data);
        log::trace!("{:#?}", entry);
    }

    bench_map.into_iter().map(|(_, v)| v).collect()
}

pub fn graph_benchmarks(benchmarks: &Vec<Benchmark>) -> View<SvgsvgElement> {
    let mut graph = (View::element_ns("svg", "http://www.w3.org/2000/svg") as View<SvgsvgElement>)
        .attribute("width", "960")
        .attribute("height", "540")
        .attribute("viewBox", "0 0 960 540")
        .attribute("class", "embed-responsive-item");

    let mut benchmarks = process_benchmarks(benchmarks);
    benchmarks.sort_by(|bencha, benchb| {
        let a = bencha.max_bench_len().round() as u32;
        let b = benchb.max_bench_len().round() as u32;
        let time_ord = a.cmp(&b);
        if bencha.error.is_some() {
            std::cmp::Ordering::Greater
        } else if benchb.error.is_some() {
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
