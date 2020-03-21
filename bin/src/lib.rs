use log::{trace, Level};
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;
use web_sys::{KeyboardEvent, SvgsvgElement};

mod bench_runner;
use bench_runner::{BenchRunner, Benchmark};

mod framework_card;
use framework_card::{all_cards, FrameworkCard, FrameworkState};

mod graph;
mod store;


#[cfg(test)]
mod bench_tests {
  extern crate wasm_bindgen_test;

  use mogwai::prelude::*;
  use wasm_bindgen::UnwrapThrowExt;
  use wasm_bindgen_test::wasm_bindgen_test_configure;
  use wasm_bindgen_test::*;

  use todo_mvc_bench_lib::wait_for;


  wasm_bindgen_test_configure!(run_in_browser);


  fn wait_and_build_div(millis: i32, id: &str, class: &str) {
    let id: String = id.into();
    let class: String = class.into();
    timeout(millis, move || {
      div().id(&id).class(&class).run().unwrap_throw();
      false
    });
  }


  #[wasm_bindgen_test]
  async fn test_can_wait_for_one() {
    wait_and_build_div(1000, "my_div", "");
    let found_el =
      wait_for(2000, || document().get_element_by_id("my_div")).await;
    assert!(found_el.is_ok());
    let found_el = found_el.unwrap();
    assert!(found_el.elapsed >= 1000.0 && found_el.elapsed < 2000.0);
  }

  #[wasm_bindgen_test]
  async fn test_can_wait_for_all() {
    wait_and_build_div(1000, "my_div_a", "my_div");
    wait_and_build_div(1000, "my_div_b", "my_div");
    wait_and_build_div(1000, "my_div_c", "my_div");
    let found_el =
      wait_for(2000, || {
      document()
        .query_selector_all(".my_div")
        .ok()
        .map(|list| if list.length() > 0 { Some(list) } else { None })
        .flatten()
    })
    .await;
    assert!(found_el.is_ok());
    let found_el = found_el.unwrap();
    assert!(found_el.elapsed >= 1000.0 && found_el.elapsed < 2000.0);
    assert!(found_el.found.length() == 3)
  }
}


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


#[derive(Clone)]
pub enum In {
  Startup,
  Container(HtmlElement),
  AvgOverTimesChange(Event),
  SoloFramework(String),
  ClickedRun,
  StepDisabled(bool),
  SuiteCompleted(Benchmark),
}


pub struct App {
  is_stepping: bool,
  cards: Vec<GizmoComponent<FrameworkCard>>,
  bench_runner: GizmoComponent<BenchRunner>,
  container: Option<HtmlElement>,
  benchmarks: Vec<Benchmark>,
  framework_index: Option<usize>,
  avg_times: u32,
  current_run: u32,
  graph: Option<Gizmo<SvgsvgElement>>,
}


impl App {
  pub fn new() -> Self {
    let cards =
      all_cards()
      .into_iter()
      .map(|card| card.into_component())
      .collect::<Vec<_>>();

    let bench_runner = BenchRunner::new().into_component();

    App {
      is_stepping: false,
      container: None,
      avg_times: 1,
      current_run: 1,
      cards,
      bench_runner,
      benchmarks: vec![],
      framework_index: None,
      graph: None,
    }
  }

  /// Possibly initialize the bench runner before running a new suite of steps.
  fn init_runner(&mut self) -> Option<String> {
    // Causes the graph to be dropped (also from the DOM).
    self.graph = None;
    let container = self.container.as_ref().expect("no container");
    let _ = container.append_child(&self.bench_runner);
    // Set all the cards to "ready"
    for card in self.cards.iter_mut() {
      card.update(&framework_card::In::ChangeState(FrameworkState::Ready));
    }
    // Update the framework that will be running and initialize the bench runner
    if let Some(framework) = self.get_next_framework() {
      framework
        .update(&framework_card::In::ChangeState(FrameworkState::Running));
      let framework = framework.with_state(|f| f.clone());
      self.bench_runner.update(&bench_runner::In::InitBench {
        url: framework.url.clone(),
        create_todo_method: framework.create_todo_method.clone(),
        name: framework.name.clone(),
        language: framework.framework_attribute("language").clone(),
      });
      Some(framework.name.clone())
    } else {
      None
    }
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

  fn get_current_framework(
    &mut self,
  ) -> Option<&mut GizmoComponent<FrameworkCard>> {
    if self.framework_index.is_none() {
      return None;
    }

    let index = self.framework_index.unwrap();
    self.cards.get_mut(index)
  }

  fn get_next_framework(
    &mut self,
  ) -> Option<&mut GizmoComponent<FrameworkCard>> {
    self.increment_framework_index();
    if self.framework_index.is_none() {
      return None;
    }
    let start_ndx =
      self
      .framework_index
      .expect("get_next_framework_url but all suites are done");
    let mut found_index = None;
    'find_index: for index in start_ndx..self.cards.len() {
      trace!("finding next framework at index: {}", index);
      let card = self.cards.get(index).unwrap_throw();
      if card.with_state(|c| c.is_enabled) {
        found_index = Some(index);
        break 'find_index;
      }
    }

    self.framework_index = found_index.clone();

    if let Some(index) = found_index {
      self.cards.get_mut(index)
    } else {
      None
    }
  }
}


#[derive(Clone)]
pub enum Out {
  IframeSrc(String),
  RunningFramework(String, (u32, u32)),
  SetAvgTimesValue(String),
  StepDisabled(bool),
  RunDisabled(bool),
  SuiteCompleted(Benchmark),
  SuiteFailed(Benchmark),
}


impl Component for App {
  type ModelMsg = In;
  type ViewMsg = Out;
  type DomNode = HtmlElement;

  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    tx: &Transmitter<Self::ViewMsg>,
    sub: &Subscriber<Self::ModelMsg>,
  ) {
    match msg {
      In::Startup => {
        trace!("startup");
        sub.subscribe_filter_map(&self.bench_runner.recv, |msg| match msg {
          bench_runner::Out::Done(benchmark) => {
            Some(In::SuiteCompleted(benchmark.clone()))
          }
          bench_runner::Out::StepDisabled(is_disabled) => {
            Some(In::StepDisabled(*is_disabled))
          }
          _ => None,
        });

        for card in self.cards.iter() {
          sub.subscribe_filter_map(&card.recv, |msg| match msg {
            framework_card::Out::Solo(name) => {
              Some(In::SoloFramework(name.clone()))
            }
            _ => None,
          })
        }
      }

      In::AvgOverTimesChange(event) => {
        let may_input =
          event
          .target()
          .map(|t| t.clone().unchecked_into::<HtmlInputElement>());
        if let Some(input) = may_input {
          if let Ok(times) = input.value().trim().parse::<u32>() {
            self.avg_times = times;
          } else {
            let times = format!("{}", self.avg_times);
            tx.send(&Out::SetAvgTimesValue(times));
          }
        }

        if let Some(event) = event.dyn_ref::<KeyboardEvent>() {
          if event.key() == "Enter" {
            sub.send_async(async {In::ClickedRun});
          }
        }
      }

      In::SoloFramework(name) => {
        for card in self.cards.iter_mut() {
          card.update(&framework_card::In::IsEnabled(
            card.with_state(|c| &c.name == name),
          ));
        }
      }

      In::Container(el) => {
        // now that we have the test and results container, we can try to read
        // any previous benchmarks and show them here.
        if let Ok(benchmarks) = store::read_benchmarks() {
          let graph = graph::graph_benchmarks(&benchmarks);
          el.append_child(&graph)
            .expect("could not append graph of previous results");
          self.graph = Some(graph);
        }
        self.container = Some(el.clone());
      }

      In::ClickedRun => {
        self.is_stepping = false;
        // The current bench run is 1
        if let Some(name) = self.init_runner() {
          tx.send(&Out::RunningFramework(name, (self.current_run, self.avg_times)));
        }
        self.bench_runner.update(&bench_runner::In::Step);
        tx.send(&Out::RunDisabled(true));
      }

      In::StepDisabled(is_disabled) => {
        tx.send(&Out::StepDisabled(*is_disabled));
      }

      In::SuiteCompleted(benchmark) => {
        trace!("suite completed");
        tx.send(&Out::RunDisabled(false));
        self.benchmarks.push(benchmark.clone());

        let component_card =
          self.get_current_framework().expect("no current framework");
        let card_state =
          if let Some(msg) = benchmark.failed_message.as_ref() {
          FrameworkState::Erred(msg.clone())
        } else {
          FrameworkState::Done
        };
        component_card.update(&framework_card::In::ChangeState(card_state));

        let may_framework =
          if let Some(framework) = self.get_next_framework() {
          framework
            .update(&framework_card::In::ChangeState(FrameworkState::Running));
          Some(framework.with_state(|f| f.clone()))
        } else {
          None
        };

        if let Some(framework) = may_framework {
          self.bench_runner.update(&bench_runner::In::InitBench {
            url: framework.url.clone(),
            create_todo_method: framework.create_todo_method.clone(),
            name: framework.name.clone(),
            language: framework.framework_attribute("language").clone(),
          });
          tx.send(&Out::RunningFramework(framework.name.clone(), (self.current_run, self.avg_times)));
          if !self.is_stepping {
            self.bench_runner.update(&bench_runner::In::Step);
          }
        } else {
          // If we have some more bench runs to average, do them!
          if self.current_run < self.avg_times {
            self.current_run += 1;
            sub.send_async(async {In::ClickedRun});
          } else {
            self.current_run = 1;

            let benchmarks = self.benchmarks.clone();
            // Write the benchmarks to local storage if possible
            let _ = store::write_items(&benchmarks);
            // Graph them
            let graph = graph::graph_benchmarks(&benchmarks);

            self
              .bench_runner
              .parent_node()
              .into_iter()
              .for_each(|parent| {
                let _ = parent.remove_child(&self.bench_runner);
              });

            let container = self.container.as_ref().expect("no container!");
            let _ = container.append_child(&graph);

            self.graph = Some(graph);

            self.framework_index = None;
            self.benchmarks = vec![];

            tx.send(&Out::RunningFramework("".into(), (self.current_run, self.avg_times)));
            trace!("done.");
          }
        }
      }
    }
  }

  fn view(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>,
  ) -> Gizmo<HtmlElement> {
    trace!("view");
    div()
      .id("main")
      .class("container-fluid")
      .with(
        nav()
          .class("navbar navbar-expand-lg navbar-light bg-light rounded-sm mt-2 mb-4")
          .with(
            a()
              .attribute("href", "https://github.com/schell/todo-mvc-bench")
              .text("schell's todo-mvc-bench")
          )
          .with(
            ul()
              .class("navbar-nav ml-2 mr-auto")
              .with(
                li()
                  .class("nav-item mr-1")
                  .with(
                    span()
                      .rx_text(
                        "",
                        rx.branch_filter_map(|msg| match msg {
                          Out::RunningFramework(name, _) => Some(name.clone()),
                          _ => None,
                        }),
                      )
                  )
              )
              .with(
                li()
                  .class("nav-item")
                  .with(
                    span()
                      .rx_text(
                        "",
                        rx.branch_filter_map(|msg| match msg {
                          Out::RunningFramework(_, (n, of)) => Some(
                            format!("run #{} of {}", n, of)
                          ),
                          _ => None,
                        }),
                      )
                  )
              )
          )
          .with(
            div()
              .class("input-group col-2")
              .with(
                div()
                  .class("input-group-prepend")
                  .with(
                    span()
                      .class("input-group-text")
                      .text("avg over")
                  )
              )
              .with(
                input()
                  .attribute("type", "text")
                  .class("form-control")
                  .attribute("placeholder", "1")
                  .rx_value(
                    "",
                    rx.branch_filter_map(|msg| match msg {
                      Out::SetAvgTimesValue(val) => Some(val.clone()),
                      _ => None,
                    }),
                  )
                  .tx_on(
                    "change",
                    tx.contra_map(|event: &Event| In::AvgOverTimesChange(event.clone())),
                  )
                  .tx_on(
                    "keyup",
                    tx.contra_filter_map(|event: &Event| {
                      let event = event.dyn_ref::<KeyboardEvent>()?;
                      if event.key() == "Enter" {
                        Some(In::AvgOverTimesChange(event.unchecked_ref::<Event>().clone())) 
                      } else {
                        None
                      }
                    })
                  ),
              )
              .with(
                div().class("input-group-append").with(
                  button()
                    .id("run_button")
                    .class("btn btn-primary")
                    .text("Run")
                    .tx_on("click", tx.contra_map(|_| In::ClickedRun))
                    .rx_boolean_attribute(
                      "disabled",
                      false,
                      rx.branch_filter_map(|msg| match msg {
                        Out::RunDisabled(disabled) => Some(*disabled),
                        _ => None,
                      }),
                    ),
                ),
              ),
          ),
      )
      .with(
        div()
          .class("container")
          .with(
            div()
              .class("row embed-responsive embed-responsive-16by9 mb-4")
              .tx_post_build(
                tx.contra_map(|el: &HtmlElement| In::Container(el.clone())),
              ),
          )
          .with(div().class("row").with({
            let card_container = div().class("card-deck mb-3 text-center");
            for card in self.cards.iter() {
              let _ = card_container.append_child(card);
            }
            card_container
          })),
      )
  }
}


#[wasm_bindgen]
pub fn bench() -> Result<(), JsValue> {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(Level::Trace).unwrap();

  App::new().into_component().run_init(vec![In::Startup])
}
