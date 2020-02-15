extern crate log;
extern crate console_log;
extern crate console_error_panic_hook;
extern crate mogwai;
extern crate serde;
extern crate serde_json;
extern crate todo_mvc_bench_lib;

use log::{Level,trace};
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

use todo_mvc_bench_lib::{
  wait_for_element,
  find::FoundElement,
  framework_card::FrameworkCard
};


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


pub enum BenchStep {
  Load(String),
  AwaitTodoInput
}


impl BenchStep {
  fn steps_from_card(card:&FrameworkCard) -> Vec<BenchStep> {
    vec![
      BenchStep::Load(card.url.clone()),
      BenchStep::AwaitTodoInput
    ]
  }

  fn to_step_string(&self) -> String {
    match self {
      BenchStep::Load(src) => {
        format!("load {}", src)
      }

      BenchStep::AwaitTodoInput => {
        "Await new-todo input".into()
      }

    }
  }
}


pub struct BenchMark {
  // millis it took to load the iframe
  pub initial_load: f64,
  pub await_todo_time: u32
}


pub struct InFlightBenchMark {
  pub load_started_at: f64,
  pub load_ended_at: f64,
  pub await_todo_time: u32
}


impl InFlightBenchMark {
  pub fn new() -> Self {
    InFlightBenchMark {
      load_started_at: 0.0,
      load_ended_at: 0.0,
      await_todo_time: 0,
    }
  }

  pub fn into_benchmark(self) -> BenchMark {
    BenchMark {
      initial_load: self.load_ended_at - self.load_started_at,
      await_todo_time: self.await_todo_time
    }
  }
}


pub struct App {
  cards: Vec<GizmoComponent<FrameworkCard>>,
  steps: Vec<BenchStep>,
  current_benchmark: InFlightBenchMark,
  may_current_step: Option<BenchStep>,
  _benchmarks: Vec<BenchMark>,
  may_todo_input: Option<FoundElement>
}


impl App {
  pub fn new() -> Self {
    let mut cards = vec![
      FrameworkCard::new(
        "mogwai",
        "0.1.5",
        "rust",
        "https://schell.github.io/mogwai/todomvc/",
        &[
          ("has vdom", false),
          ("is elm like", true)
        ],
        true
      ).into_component()
    ];
    cards
      .iter_mut()
      .for_each(|card| {
        card.build();
      });

    let steps =
      cards
      .iter()
      .flat_map(|card| card.with_state(|st| BenchStep::steps_from_card(st)))
      .collect::<Vec<_>>();

    App {
      cards,
      steps,
      current_benchmark: InFlightBenchMark::new(),
      may_current_step: None,
      _benchmarks: vec![],
      may_todo_input: None
    }
  }

  fn next_step(&self) -> Option<&BenchStep> {
    self.steps.first()
  }

  fn send_next_step(&self, tx: &Transmitter<Out>) {
    self
      .next_step()
      .map(|step| step.to_step_string())
      .into_iter()
      .for_each(|step_str| tx.send(&Out::NextStep(step_str)));
  }

  fn start_step(&mut self, tx: &Transmitter<Out>, sub: &Subscriber<In>, step:BenchStep) {
    match &step {
      BenchStep::Load(src) => {
        tx.send(&Out::IframeSrc(src.clone()));
        let now =
          window()
          .performance().unwrap_throw()
          .now();
        self.current_benchmark.load_started_at = now;
      }

      BenchStep::AwaitTodoInput => {
        sub.send_async(async {
          let may_input = wait_for_element("new-todo", 1000).await;
          if let Some(input) = may_input {
            In::TodoInputFound(input)
          } else {
            In::TodoInputNotFound
          }
        });
      }
    }

    self.may_current_step = Some(step);
  }

  fn complete_current_step(&mut self, tx: &Transmitter<Out>) {
    let step =
      self
      .may_current_step
      .take()
      .expect("no current step");

    match &step {
      BenchStep::Load(_) => {
        let now =
          window()
          .performance().unwrap_throw()
          .now();

        self.current_benchmark.load_ended_at = now;
        trace!("initial load: {}millis", self.current_benchmark.load_ended_at - self.current_benchmark.load_started_at);
      }

      BenchStep::AwaitTodoInput => {
        let found_todo_input =
          self
          .may_todo_input
          .as_ref()
          .expect("no todo input at end of await todo step");
        self.current_benchmark.await_todo_time =
          found_todo_input.elapsed;
        trace!("await todo input: {}millis", found_todo_input.elapsed);
      }
    }

    self.send_next_step(tx);
  }
}


#[derive(Clone)]
pub enum In {
  Startup,
  ClickedStep,
  ClickedRun,
  IframeLoaded,
  TodoInputFound(FoundElement),
  TodoInputNotFound
}

#[derive(Clone)]
pub enum Out {
  IframeSrc(String),
  NextStep(String)
}


impl Out {
  fn iframe_src(&self) -> Option<Option<String>> {
    match self {
      Out::IframeSrc(src) => {
        Some(Some(src.clone()))
      }
      _ => { None }
    }
  }

  fn next_step_string(&self) -> Option<String> {
    match self {
      Out::NextStep(step) => {
        Some(step.clone())
      }
      _ => { None }
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
        self.send_next_step(tx);
      }
      In::ClickedStep => {
        let has_step =
          self
          .next_step()
          .is_some();
        if has_step {
          let step =
            self
            .steps
            .remove(0);
          self.start_step(tx, sub, step);
          self.send_next_step(tx);
        }
      }
      In::ClickedRun => {
      }
      In::IframeLoaded => {
        // for some reason the iframe sends off a loaded event at the beginning
        if self.may_current_step.is_some() {
          self.complete_current_step(tx);
        }
      }
      In::TodoInputFound(found_el) => {
        self.may_todo_input = Some(found_el.clone());
        self.complete_current_step(tx);
      }
      In::TodoInputNotFound => {
        trace!("todo input not found!");
        // TODO: Mark current framework card as erred.
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
            .class("btn btn-outline-primary mr-1")
            .text("Step")
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
              .with(
                iframe()
                  .class("todo-src embed-responsive-item")
                  .rx_attribute(
                    "src",
                    None,
                    rx.branch_filter_map(|msg| msg.iframe_src())
                  )
                  .tx_on("load", tx.contra_map(|_| In::IframeLoaded))
              )
              .rx_style(
                "display",
                "none",
                rx.branch_filter_map(|msg| {
                  if let Out::IframeSrc(_) = msg {
                    Some("block".into())
                  } else {
                    None
                  }
                })
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

  use todo_mvc_bench_lib::{
    find::FoundElement,
    wait_for_element
  };


  wasm_bindgen_test_configure!(run_in_browser);


  fn wait_and_build_div(millis: i32, id: &str) {
    let id:String = id.into();
    timeout(millis, move || {
      div()
        .id(&id)
        .build().unwrap_throw()
        .run().unwrap_throw();
      false
    });
  }


  #[wasm_bindgen_test]
  async fn test_can_wait() {
    wait_and_build_div(1000, "my_div");
    let found_el:Option<FoundElement> = wait_for_element("my_div", 2000).await;
    assert!(found_el.is_some());
    let found_el = found_el.unwrap();
    assert!(found_el.elapsed >= 1000 && found_el.elapsed < 2000);
  }
}
