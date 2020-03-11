use log::{error, trace};
use mogwai::prelude::*;
use web_sys::{
  HtmlIFrameElement
};

use todo_mvc_bench_lib::{
  wait_for,
  find::Found,
  async_event::{EventResult, wait_for_event_on},
};

use super::framework_card::CreateTodoMethod;


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
}


impl BenchStep {
  fn steps(url:&str) -> Vec<BenchStep> {
    vec![
      BenchStep::Load(url.into()),
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
    }
  }
}


#[derive(Clone)]
pub struct Benchmark {
  pub load: f64,
  pub await_todo_time: f64,
  pub todos_creation: f64,
  pub todos_creation_confirmation: f64,
  pub todos_completed: f64,
  pub todos_deleted: f64,
  pub todos_deleted_confirmation: f64,
  pub failed_message: Option<String>
}


impl Benchmark {
  pub fn new() -> Self {
    Benchmark {
      load: 0.0,
      await_todo_time: 0.0,
      todos_creation: 0.0,
      todos_creation_confirmation: 0.0,
      todos_completed: 0.0,
      todos_deleted: 0.0,
      todos_deleted_confirmation: 0.0,
      failed_message: None
    }
  }

  pub fn total(&self) -> f64 {
    let mut total = 0.0;
    total += self.load;
    total += self.await_todo_time;
    total += self.todos_creation;
    total += self.todos_creation_confirmation;
    total += self.todos_completed;
    total += self.todos_deleted;
    total += self.todos_deleted_confirmation;
    total
  }
}


#[derive(Clone)]
pub enum In {
  Iframe(HtmlIFrameElement),
  InitBench(String, CreateTodoMethod),
  // Step through one benchmark
  Step,
  StepCompleted(StepRunner),
}


#[derive(Clone)]
pub enum Out {
  IframeSrc(String),
  StepDisabled(bool),
  Failed(Benchmark),
  Done(Benchmark)
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
}


/// Handles running one step
#[derive(Clone)]
pub struct StepRunner {
  iframe: HtmlIFrameElement,
  may_todo_input: Option<HtmlInputElement>,
  create_todo_method: CreateTodoMethod,
  benchmark: Benchmark,
}


impl StepRunner {
  async fn execute_step(&mut self, tx: &Transmitter<Out>, step:BenchStep) {
    trace!("starting step: {}", step.to_step_string());
    tx.send(&Out::StepDisabled(true));

    let document =
      self
      .iframe
      .content_document()
      .expect("no iframe document");

    let may_err:Option<String> =
      match &step {
        BenchStep::Load(src) => {
          trace!("  waiting for iframe load complete");
          tx.send(&Out::IframeSrc(src.clone()));
          let res:EventResult =
            wait_for_event_on("load", &self.iframe)
            .await;
          self.benchmark.load = res.elapsed;
          trace!("  load complete {}ms", res.elapsed);
          None
        }
        BenchStep::AwaitTodoInput => {
          let may_input = wait_for(
            1000,
            move || document.query_selector("#new-todo").ok().flatten()
          ).await;

          if let Some(Found{elapsed, found: input}) = may_input {
            self.benchmark.await_todo_time = elapsed;
            trace!("await todo input: {}millis", elapsed);

            self.may_todo_input = Some(
              input
                .dyn_into::<HtmlInputElement>()
                .expect("is not an input")
            );

            None
          } else {
            Some("todo input not found!".into())
          }
        }
        BenchStep::EnterTodos => {
          let input =
            self
            .may_todo_input
            .as_ref()
            .expect("no todo input");
          input
            .focus()
            .expect("could not focus input");
          let perf =
            window()
            .performance()
            .expect("no peformance object");
          let start = perf.now();
          for i in 0 ..= 99 {
            input.set_value(&format!("Something to do {}", i));
            self
              .create_todo_method
              .dispatch_events(&document, &input);
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

          if let Some(Found{elapsed: time_to_confirm, ..}) = found {
            let time_to_create = end - start;
            trace!("time to create:  {}millis", time_to_create);
            trace!("time to confirm: {}millis", time_to_confirm);
            self.benchmark.todos_creation = time_to_create;
            self.benchmark.todos_creation_confirmation = time_to_confirm;
            None
          } else {
            Some("todos could not be created".into())
          }
        }
        BenchStep::CompleteTodos => {
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

          if let Some(Found{elapsed, ..}) = found {
            trace!("time to complete: {}millis", elapsed);
            self.benchmark.todos_completed = elapsed;
            None
          } else {
            Some("todos could not be completed".into())
          }
        }
        BenchStep::DeleteTodos => {
          let document_for_delete = document.clone();
          let found = wait_for(
            5000,
            move || {
              document_for_delete
                .query_selector_all(".destroy")
                .ok()
                .map(|list| {
                  if list.length() == 0 {
                    Some(())
                  } else {
                    let el =
                      list
                      .get(0)
                      .expect("could not get todo destroy button")
                      .dyn_into::<HtmlElement>()
                      .expect("could not cast todo destroy button");
                    el.click();
                    None
                  }
                })
                .flatten()
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
              trace!("time to delete:  {}millis", time_to_delete);
              trace!("time to confirm: {}millis", time_to_confirm);
              self.benchmark.todos_deleted = time_to_delete;
              self.benchmark.todos_deleted_confirmation = time_to_confirm;
              None
            } else {
              Some("cannot confirm todos deletion".into())
            }
          } else {
            Some("todo destroy buttons not found".into())
          }
        }
      };

    if let Some(err) = may_err {
      error!("{}", err);
      self.benchmark.failed_message = Some(err.clone());
      tx.send(&Out::Failed(self.benchmark.clone()));
    }

    tx.send(&Out::StepDisabled(false));
  }
}


/// Handles running the benchmarks for one framework step by step
pub struct BenchRunner {
  iframe: Option<HtmlIFrameElement>,
  create_todo_method: CreateTodoMethod,
  steps: Vec<BenchStep>,
  step_runner: Option<StepRunner>
}


impl BenchRunner {
  pub fn new() -> Self {
    BenchRunner {
      iframe: None,
      create_todo_method: CreateTodoMethod::Change,
      steps: vec![],
      step_runner: None
    }
  }

  pub fn has_steps(&self) -> bool {
    self.steps.len() > 0
  }
}


impl Component for BenchRunner {
  type ModelMsg = In;
  type ViewMsg = Out;

  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    tx: &Transmitter<Self::ViewMsg>,
    sub: &Subscriber<Self::ModelMsg>
  ) {
    match msg {
      In::Iframe(iframe) => {
        self.iframe = Some(iframe.clone());
      }

      In::InitBench(url, create_todo_method) => {
        self.steps = BenchStep::steps(url);
        self.create_todo_method = create_todo_method.clone();
      }

      In::Step => {
        let iframe =
          self
          .iframe
          .as_ref()
          .expect("no iframe");

        let mut step_runner =
          self
          .step_runner
          .take()
          .unwrap_or(
            StepRunner {
              iframe: iframe.clone(),
              create_todo_method: self.create_todo_method.clone(),
              may_todo_input: None,
              benchmark: Benchmark::new()
            }
          );
        step_runner.create_todo_method = self.create_todo_method.clone();

        if !self.steps.is_empty() {
          let step = self.steps.remove(0);

          let tx = tx.clone();
          sub.send_async(async move {
            step_runner.execute_step(&tx, step).await;
            In::StepCompleted(step_runner)
          });
        }
      }

      In::StepCompleted(runner) => {
        if runner.benchmark.failed_message.is_some() {
          self.steps = vec![];
        }

        self.step_runner =
          if self.steps.is_empty() {
            tx.send(&Out::Done(runner.benchmark.clone()));
            None
          } else {
            Some(runner.clone())
          };
      }
    }
  }

  fn builder(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>
  ) -> GizmoBuilder {
    iframe()
      .class("todo-src embed-responsive-item")
      .rx_attribute(
        "src",
        None,
        rx.branch_filter_map(|msg| msg.iframe_src())
      )
      .tx_post_build(tx.contra_map(|el:&HtmlElement| {
        In::Iframe(
          el
            .clone()
            .dyn_into::<HtmlIFrameElement>()
            .unwrap_throw()
        )
      }))
  }
}
