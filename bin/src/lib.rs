use log::{Level,trace};
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

mod bench_runner;
use bench_runner::{Benchmark, BenchRunner};

mod framework_card;
use framework_card::{
    all_cards,
    FrameworkCard,
    FrameworkState,
};



// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


#[derive(Clone)]
pub enum In {
  Startup,
  ClickedStep,
  ClickedRun,
  StepDisabled(bool),
  SuiteFailed(Benchmark),
  SuiteCompleted(Benchmark)
}


pub struct App {
  is_stepping: bool,
  cards: Vec<GizmoComponent<FrameworkCard>>,
  bench_runner: GizmoComponent<BenchRunner>,
  benchmarks: Vec<(String, Benchmark)>,
  framework_index: Option<usize>
}


impl App {
  pub fn new() -> Self {
    let mut cards =
      all_cards()
      .into_iter()
      .map(|card | card.into_component())
      .collect::<Vec<_>>();
    cards
      .iter_mut()
      .for_each(|card| {
        card.build();
      });

    let mut bench_runner = BenchRunner::new().into_component();
    bench_runner.build();

    App {
      is_stepping: false,
      cards,
      bench_runner,
      benchmarks: vec![],
      framework_index: None
    }
  }

  fn push_benchmark(
    &mut self,
    benchmark:Benchmark
  ) -> &mut GizmoComponent<FrameworkCard> {
    let index = self.framework_index.expect("no framework index");
    let card = self.cards.get_mut(index).expect("no framework");
    let name = card.with_state(|c| c.name.clone());
    self.benchmarks.push((name, benchmark.clone()));
    card
  }

  fn increment_framework_index(&mut self) {
    if let Some(index) = self.framework_index.as_mut() {
      *index += 1;
      if *index == self.cards.len() {
        self.framework_index = None;
      }
    } else {
      self.framework_index = Some(0);
    }
  }

  fn get_next_framework(&mut self) -> Option<FrameworkCard> {
    self.increment_framework_index();
    if self.framework_index.is_none() {
      return None;
    }
    let start_ndx =
      self
      .framework_index
      .expect("get_next_framework_url but all suites are done");
    for index in start_ndx .. self.cards.len() {
      trace!("finding next framework at index: {}", index);
      let card =
        self
        .cards
        .get(index)
        .unwrap_throw()
        .with_state(|c| c.clone());
      self.framework_index = Some(index);
      if card.is_enabled {
        return Some(card.clone());
      }
    }
    None
  }
}


#[derive(Clone)]
pub enum Out {
  IframeSrc(String),
  NextStep(String),
  StepDisabled(bool),
  SuiteCompleted(Benchmark),
  SuiteFailed(Benchmark)
}


impl Out {
  fn next_step_string(&self) -> Option<String> {
    match self {
      Out::NextStep(step) => {
        Some(step.clone())
      }
      _ => { None }
    }
  }

  fn step_disabled(&self) -> Option<bool> {
    if let Out::StepDisabled(is_disabled) = self {
      Some(*is_disabled)
    } else {
      None
    }
  }
}


impl Component for App {
  type ModelMsg = In;
  type ViewMsg = Out;

  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    tx: &Transmitter<Self::ViewMsg>,
    sub: &Subscriber<Self::ModelMsg>
  ) {
    match msg {
      In::Startup => {
        sub.subscribe_filter_map(&self.bench_runner.recv, move |child_msg| {
          match child_msg {
            bench_runner::Out::Failed(benchmark) => {
              Some(In::SuiteFailed(benchmark.clone()))
            }
            bench_runner::Out::Done(benchmark) => {
              Some(In::SuiteCompleted(benchmark.clone()))
            }
            bench_runner::Out::StepDisabled(is_disabled) => {
              Some(In::StepDisabled(*is_disabled))
            }
            _ => { None }
          }
        });
      }
      In::ClickedStep => {
        self.is_stepping = true;
        if !self.bench_runner.with_state(|b| b.has_steps()) {
          if let Some(framework) = self.get_next_framework() {
            self.bench_runner.update(&bench_runner::In::InitBench(
              framework.url,
              framework.create_todo_method
            ));
          }
        }
        self.bench_runner.update(&bench_runner::In::Step);
      }
      In::ClickedRun => {
        self.is_stepping = false;
        if !self.bench_runner.with_state(|b| b.has_steps()) {
          if let Some(framework) = self.get_next_framework() {
            self.bench_runner.update(&bench_runner::In::InitBench(
              framework.url,
              framework.create_todo_method
            ));
          }
        }
        self.bench_runner.update(&bench_runner::In::Step);
      }
      In::StepDisabled(is_disabled) => {
        tx.send(&Out::StepDisabled(*is_disabled));
        if !is_disabled && !self.is_stepping {
          self.bench_runner.update(&bench_runner::In::Step);
        }
      }
      In::SuiteFailed(benchmark) => {
        let msg =
          benchmark
          .failed_message
          .clone()
          .unwrap_or("unknown suite failure".into());
        trace!("{}", msg);
        let card = self.push_benchmark(benchmark.clone());
        card.update(&framework_card::In::ChangeState(FrameworkState::Erred(msg)));
      }
      In::SuiteCompleted(benchmark) => {
        let _component_card = self.push_benchmark(benchmark.clone());
        if let Some(framework) = self.get_next_framework() {
          self.bench_runner.update(&bench_runner::In::InitBench(
            framework.url,
            framework.create_todo_method
          ));
          if !self.is_stepping {
            self.bench_runner.update(&bench_runner::In::Step);
          }
        } else {
          trace!("done.");
        }
      }
    }
  }

  fn builder(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>
  ) -> GizmoBuilder {
    div()
      .id("main")
      .class("container-fluid")
      .with(
        nav()
          .class("navbar navbar-light bg-light rounded-sm mt-2 mb-4")
          .with(
            div()
              .class("navbar-brand")
              .text("schell's todo-mvc-bench")
          )
        .with(
          ul()
            .class("navbar-nav mr-auto ml-auto")
            .with(
              li()
                .class("nav-item")
                .with(
                  dl()
                    .with(
                      dt()
                        .text("Next step")
                    )
                    .with(
                      dd()
                        .rx_text(
                          "",
                          rx.branch_filter_map(|msg| msg.next_step_string())
                        )
                    )
                )
            )
        )
        .with(
          button()
            .id("step_button")
            .class("btn btn-secondary mr-1")
            .text("Step")
            .rx_boolean_attribute(
              "disabled",
              false,
              rx.branch_filter_map(|msg| msg.step_disabled())
            )
            .tx_on("click", tx.contra_map(|_| In::ClickedStep))
        )
        .with(
          button()
            .id("run_button")
            .class("btn btn-primary")
            .text("Run")
            .tx_on("click", tx.contra_map(|_| In::ClickedRun))
        )
      )
      .with(
        div()
          .class("container")
          .with(
            div()
              .class("row embed-responsive embed-responsive-16by9 mb-4")
              .with_pre_built(
                self
                  .bench_runner
                  .gizmo
                  .as_ref()
                  .expect("no bench runner")
                  .html_element
                  .clone()
              )
          )
          .with(
            div()
              .class("row")
              .with(
                div()
                  .class("card-deck mb-3 text-center")
                  .with_gizmos(
                    self
                      .cards
                      .iter()
                      .map(|gc:&GizmoComponent<_>| {
                        gc.gizmo
                          .as_ref()
                          .expect("gizmo is not built")
                      })
                      .collect::<Vec<_>>()
                  )
              )
          )
      )
  }
}


#[wasm_bindgen]
pub fn bench() -> Result<(), JsValue> {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(Level::Trace)
    .unwrap();

  App::new()
    .into_component()
    .run_init(vec![In::Startup])
}


#[cfg(test)]
mod bench_tests {
  extern crate wasm_bindgen_test;

  use wasm_bindgen::UnwrapThrowExt;
  use wasm_bindgen_test::*;
  use mogwai::prelude::*;
  use wasm_bindgen_test::wasm_bindgen_test_configure;

  use todo_mvc_bench_lib::wait_for;


  wasm_bindgen_test_configure!(run_in_browser);


  fn wait_and_build_div(millis: i32, id: &str, class: &str) {
    let id:String = id.into();
    let class:String = class.into();
    timeout(millis, move || {
      div()
        .id(&id)
        .class(&class)
        .build().unwrap_throw()
        .run().unwrap_throw();
      false
    });
  }


  #[wasm_bindgen_test]
  async fn test_can_wait_for_one() {
    wait_and_build_div(1000, "my_div", "");
    let found_el = wait_for(
      2000,
      || document().get_element_by_id("my_div")
    ).await;
    assert!(found_el.is_some());
    let found_el = found_el.unwrap();
    assert!(found_el.elapsed >= 1000.0 && found_el.elapsed < 2000.0);
  }

  #[wasm_bindgen_test]
  async fn test_can_wait_for_all() {
    wait_and_build_div(1000, "my_div_a", "my_div");
    wait_and_build_div(1000, "my_div_b", "my_div");
    wait_and_build_div(1000, "my_div_c", "my_div");
    let found_el = wait_for(
      2000,
      || {
        document()
          .query_selector_all(".my_div")
          .ok()
          .map(|list| {
            if list.length() > 0 {
              Some(list)
            } else {
              None
            }
          })
          .flatten()
      }
    ).await;
    assert!(found_el.is_some());
    let found_el = found_el.unwrap();
    assert!(found_el.elapsed >= 1000.0 && found_el.elapsed < 2000.0);
    assert!(found_el.found.length() == 3)
  }
}
