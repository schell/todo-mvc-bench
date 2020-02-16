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
use web_sys::{
  Element,
  Event,
  Document,
  HtmlIFrameElement
};

use todo_mvc_bench_lib::{
  wait_for,
  find::Found,
  framework_card::{
    all_cards,
    FrameworkCard
  }
};


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


pub enum BenchStep {
  Load(String),
  AwaitTodoInput,
  EnterTodos,
  CompleteTodos,
  DeleteTodos,
  Done
}


impl BenchStep {
  fn steps_from_card(card:&FrameworkCard) -> Vec<BenchStep> {
    vec![
      BenchStep::Load(card.url.clone()),
      BenchStep::AwaitTodoInput,
      BenchStep::EnterTodos,
      BenchStep::CompleteTodos,
      BenchStep::DeleteTodos,
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
      BenchStep::EnterTodos => {
        "Enter todos".into()
      }
      BenchStep::CompleteTodos => {
        "Complete todos".into()
      }
      BenchStep::DeleteTodos => {
        "Delete todos".into()
      }
      BenchStep::Done => {
        "Done!".into()
      }
    }
  }
}


pub struct Benchmark {
  pub load_started_at: f64,
  pub load_ended_at: f64,
  pub await_todo_time: f64,
  pub todos_creation: f64,
  pub todos_creation_confirmation: f64,
  pub todos_completed: f64,
  pub todos_deleted: f64,
  pub todos_deleted_confirmation: f64
}


impl Benchmark {
  pub fn new() -> Self {
    Benchmark {
      load_started_at: 0.0,
      load_ended_at: 0.0,
      await_todo_time: 0.0,
      todos_creation: 0.0,
      todos_creation_confirmation: 0.0,
      todos_completed: 0.0,
      todos_deleted: 0.0,
      todos_deleted_confirmation: 0.0,
    }
  }
}


#[derive(Clone)]
pub enum In {
  Startup,
  ClickedStep,
  ClickedRun,
  NextStep,
  IframeLoaded(Document),
  TodoInputFound(Found<Element>),
  TodoInputNotFound,
  TodosCreated {
    time_to_create: f64,
    time_to_confirm: f64
  },
  TodosNotCreated,
  TodosCompleted(f64),
  TodosNotCompleted,
  TodosDeleted {
    time_to_delete: f64,
    time_to_confirm: f64
  },
  TodosNotDeleted,
}


pub struct App {
  is_stepping: bool,
  cards: Vec<GizmoComponent<FrameworkCard>>,
  iframe_document: Option<Document>,
  steps: Vec<BenchStep>,
  step_suite: Vec<Vec<BenchStep>>,
  current_benchmark: Benchmark,
  may_current_step: Option<BenchStep>,
  _benchmarks: Vec<Benchmark>,
  may_todo_input: Option<Found<Element>>
}


impl App {
  pub fn new() -> Self {
    let (mut step_suite, mut cards):(Vec<_>, Vec<_>) =
      all_cards()
      .into_iter()
      .map(|card| (BenchStep::steps_from_card(&card), card.into_component()))
      .unzip();
    cards
      .iter_mut()
      .for_each(|card| {
        card.build();
      });

    let steps = step_suite.remove(0);

    App {
      is_stepping: false,
      cards,
      steps,
      step_suite,
      iframe_document: None,
      current_benchmark: Benchmark::new(),
      may_current_step: None,
      _benchmarks: vec![],
      may_todo_input: None
    }
  }

  fn next_step(&self) -> Option<&BenchStep> {
    self.steps.first()
  }

  fn step(&mut self, tx: &Transmitter<Out>, sub: &Subscriber<In>) {
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

  fn send_next_step(&self, tx: &Transmitter<Out>) {
    let step =
      self
      .next_step()
      .unwrap_or(&BenchStep::Done)
      .clone();
    let step_str = step.to_step_string();
    tx.send(&Out::NextStep(step_str));
  }

  fn get_iframe_document(&self) -> Document {
    self
      .iframe_document
      .as_ref()
      .cloned()
      .expect("can't get iframe document")
  }

  fn start_step(&mut self, tx: &Transmitter<Out>, sub: &Subscriber<In>, step:BenchStep) {
    trace!("starting step: {}", step.to_step_string());
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
        let document =
          self
          .iframe_document
          .as_ref()
          .cloned()
          .expect("no iframe document");
        sub.send_async(async move {
          let may_input = wait_for(
            1000,
            move || document.get_element_by_id("new-todo")
          ).await;
          if let Some(input) = may_input {
            In::TodoInputFound(input)
          } else {
            In::TodoInputNotFound
          }
        });
      }
      BenchStep::EnterTodos => {
        let input =
          self
          .may_todo_input
          .as_ref()
          .cloned()
          .expect("no todo input")
          .found
          .dyn_into::<HtmlInputElement>()
          .expect("can't cast input");
        input
          .focus()
          .expect("could not focus input");
        let document = self.get_iframe_document();
        sub.send_async(async move {
          let perf =
            window()
            .performance()
            .expect("no peformance object");
          let start = perf.now();
          for i in 0 ..= 99 {
            input.set_value(&format!("Something to do {}", i));
            let event =
              document
              .create_event("Event")
              .expect("could not create change event");
            event.init_event_with_bubbles_and_cancelable("change", true, true);
            input
              .dispatch_event(&event)
              .expect("could not dispatch event");
          }
          let end = perf.now();
          let found = wait_for(
            5000,
            move || {
              document
                .query_selector_all(".toggle")
                .ok()
                .map(|list| {
                  if list.length() == 100 {
                    Some(list)
                  } else {
                    None
                  }
                })
                .flatten()
            }
          ).await;
          if let Some(found) = found {
            In::TodosCreated {
              time_to_create: end - start,
              time_to_confirm: found.elapsed
            }
          } else {
            In::TodosNotCreated
          }
        });
      }
      BenchStep::CompleteTodos => {
        let document = self.get_iframe_document();
        sub.send_async(async move {
          let found = wait_for(
            5000,
            move || {
              document
                .query_selector_all(".toggle")
                .ok()
                .map(|list| -> Option<()> {
                  for i in 0..list.length() {
                    let el =
                      list
                      .get(i)
                      .expect("could not get todo toggle checkbox")
                      .dyn_into::<HtmlElement>()
                      .expect("could not cast todo toggle checkbox");
                    el.click();
                  }
                  if list.length() == 100 {
                    Some(())
                  } else {
                    None
                  }
                })
                .flatten()
            }
          ).await;
          if let Some(found) = found {
            In::TodosCompleted(found.elapsed)
          } else {
            In::TodosNotCompleted
          }
        });
      }
      BenchStep::DeleteTodos => {
        let document = self.get_iframe_document();
        sub.send_async(async move {
          let document_for_delete = document.clone();
          let found = wait_for(
            5000,
            move || {
              document_for_delete
                .query_selector_all(".destroy")
                .ok()
                .map(|list| {
                  for i in 0..list.length() {
                    let el =
                      list
                      .get(i)
                      .expect("could not get todo destroy button")
                      .dyn_into::<HtmlElement>()
                      .expect("could not cast todo destroy button");
                    el.click();
                  }
                  if list.length() == 100 {
                    Some(())
                  } else {
                    None
                  }
                })
            }
          ).await;
          if let Some(Found{elapsed:time_to_delete, ..}) = found {
            let found = wait_for(
              5000,
              move || {
                document
                  .query_selector_all(".toggle")
                  .ok()
                  .map(|list| {
                    if list.length() == 0 {
                      Some(())
                    } else {
                      None
                    }
                  })
                  .flatten()
              }
            ).await;
            if let Some(Found{elapsed:time_to_confirm, ..}) = found {
              In::TodosDeleted {
                time_to_delete,
                time_to_confirm
              }
            } else {
              In::TodosNotDeleted
            }
          } else {
            In::TodosNotDeleted
          }
        });
      }
      BenchStep::Done => {

      }
    }

    self.may_current_step = Some(step);
  }

  fn complete_current_step(
    &mut self,
    tx: &Transmitter<Out>,
    sub: &Subscriber<In>
  ) {
    let step =
      self
      .may_current_step
      .take()
      .expect("no current step");

    trace!("completed step: {}", step.to_step_string());

    if self.steps.is_empty() {
      if self.step_suite.is_empty() {
        // TODO: visualize results!
      } else {
        let steps = self.step_suite.remove(0);
        self.steps = steps;
      }
    }

    self.send_next_step(tx);
    if !self.is_stepping {
      sub.send_async(async {In::NextStep});
    }
  }
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
        self.is_stepping = true;
        self.step(tx, sub);
      }
      In::NextStep => {
        self.step(tx, sub);
      }
      In::ClickedRun => {
        self.is_stepping = false;
        self.step(tx, sub);
      }
      In::IframeLoaded(doc) => {
        // for some reason the iframe sends off a loaded event at the beginning
        if self.may_current_step.is_some() {
          let now =
            window()
            .performance().unwrap_throw()
            .now();

          self.current_benchmark.load_ended_at = now;
          trace!(
            "initial load: {}millis",
            self.current_benchmark.load_ended_at - self.current_benchmark.load_started_at
          );

          self.iframe_document = Some(doc.clone());
          self.complete_current_step(tx, sub);
        }
      }
      In::TodoInputFound(found_todo_input) => {
        self.current_benchmark.await_todo_time =
          found_todo_input.elapsed;
        trace!("await todo input: {}millis", found_todo_input.elapsed);

        self.may_todo_input = Some(found_todo_input.clone());
        self.complete_current_step(tx, sub);
      }
      In::TodoInputNotFound => {
        trace!("todo input not found!");
        // TODO: Mark current framework card as erred.
      }
      In::TodosCreated{ time_to_create, time_to_confirm } => {
        trace!("time to create:  {}millis", time_to_create);
        trace!("time to confirm: {}millis", time_to_confirm);
        self.current_benchmark.todos_creation = *time_to_create;
        self.current_benchmark.todos_creation_confirmation = *time_to_confirm;
        self.complete_current_step(tx, sub);
      }
      In::TodosNotCreated => {
        trace!("todos could not be created!");
        // TODO: Mark current framework card as erred.
      }
      In::TodosCompleted(elapsed) => {
        trace!("time to complete: {}millis", elapsed);
        self.complete_current_step(tx, sub);
      }
      In::TodosNotCompleted => {
        trace!("todos could not be completed!");
        // TODO: Mark current framework card as erred.
      }
      In::TodosDeleted{ time_to_delete, time_to_confirm } => {
        trace!("time to delete:  {}millis", time_to_delete);
        trace!("time to confirm: {}millis", time_to_confirm);
        self.current_benchmark.todos_deleted = *time_to_delete;
        self.current_benchmark.todos_deleted_confirmation = *time_to_confirm;
        self.complete_current_step(tx, sub);
      }
      In::TodosNotDeleted => {
        trace!("todos could not be deleted!");
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
                  .tx_on("load", tx.contra_map(|event:&Event| {
                    let iframe =
                      event
                      .target()
                      .expect("iframe load has no target")
                      .dyn_into::<HtmlIFrameElement>()
                      .expect("can't cast iframe");
                    let document =
                      iframe
                      .content_document()
                      .expect("can't access iframe's content_document");
                    In::IframeLoaded(document)
                  }))
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
