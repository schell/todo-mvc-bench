use log::{error, trace};
use mogwai::prelude::*;
use serde::{Deserialize, Serialize};
use web_sys::HtmlIFrameElement;

use todo_mvc_bench_lib::{
  async_event::{wait_for_event_on, EventResult},
  find::Found,
  wait, wait_for,
};

use super::framework_card::CreateTodoMethod;


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Benchmark {
  pub name: String,
  pub load: (f64, f64),
  pub await_todo: (f64, f64),
  pub todos_creation: (f64, f64),
  pub todos_completed: (f64, f64),
  pub todos_deleted: (f64, f64),
  pub failed_message: Option<String>,
  pub language: Option<String>,
}


impl Benchmark {
  pub fn new() -> Self {
    Benchmark {
      name: "unnamed".into(),
      load: (0.0, 0.0),
      await_todo: (0.0, 0.0),
      todos_creation: (0.0, 0.0),
      todos_completed: (0.0, 0.0),
      todos_deleted: (0.0, 0.0),
      failed_message: None,
      language: None,
    }
  }

  pub fn total(&self) -> f64 {
    self.todos_deleted.1
  }

  pub fn event_deltas(&self) -> Vec<(String, f64, f64)> {
    vec![
      ("load".to_string(), self.load.0, self.load.1), 
      ("await input".to_string(), self.await_todo.0, self.await_todo.1), 
      ("create todos".to_string(), self.todos_creation.0, self.todos_creation.1), 
      ("complete todos".to_string(), self.todos_completed.0, self.todos_completed.1), 
      ("delete todos".to_string(), self.todos_deleted.0, self.todos_deleted.1), 
    ]
  }
}


#[derive(Clone)]
pub enum In {
  Iframe(HtmlIFrameElement),
  InitBench {
    name: String,
    url: String,
    create_todo_method: CreateTodoMethod,
    language: Option<String>,
  },
  // Step through one benchmark
  Step,
  BenchCompleted(StepRunner),
}


#[derive(Clone)]
pub enum Out {
  IframeSrc(String),
  StepDisabled(bool),
  Done(Benchmark),
}


impl Out {
  fn iframe_src(&self) -> Option<Option<String>> {
    match self {
      Out::IframeSrc(src) => Some(Some(src.clone())),
      _ => None,
    }
  }
}


/// Handles running one step
#[derive(Clone)]
pub struct StepRunner {
  iframe: HtmlIFrameElement,
  create_todo_method: CreateTodoMethod,
  benchmark: Benchmark,
}


impl StepRunner {
  async fn execute_bench(
    &mut self,
    tx: &Transmitter<Out>,
    src: String,
  ) -> Result<(), String> {
    let perf = window().performance().ok_or("no performance")?;
    let first_perf = perf.now();
    let perf_since = |start| {
      (start - first_perf, perf.now() - first_perf)
    };
    // Load the iframe source
    trace!("{} waiting for iframe load complete", src);
    let load_start = perf.now();
    tx.send(&Out::IframeSrc(src.clone()));
    let res: EventResult = wait_for_event_on("load", &self.iframe).await;
    self.benchmark.load = perf_since(load_start);
    trace!("  load complete {}ms", res.elapsed);
    let document =
      self
      .iframe
      .content_document()
      .expect("no iframe content_document");

    // Find the todo input
    let find_todos_doc = document.clone();
    let await_todo_start = perf.now();
    let Found {found: input,..} =
      wait_for(5000, move || {
      find_todos_doc
        .query_selector("#new-todo")
        .ok()
        .flatten()
        .or(find_todos_doc.query_selector(".new-todo").ok().flatten())
    })
    .await
    .map_err(|_| "todo input not found".to_string())?;
    let todo_input: HtmlInputElement = input.unchecked_into::<HtmlInputElement>();
    self.benchmark.await_todo = perf_since(await_todo_start);
    trace!("  found todo input");

    // Enter the todos
    let todos_create_start = perf.now();
    todo_input.focus().expect("could not focus input");
    let perf = window().performance().ok_or("no performance".to_string())?;
    for i in 0..= 99 as usize {
      let value = format!("Something to do {}", i);
      todo_input.set_value(&value);
      self
        .create_todo_method
        .dispatch_events(&document, &todo_input);
    }
    self.benchmark.todos_creation = perf_since(todos_create_start);
    trace!("  created todos");

    // Find the todos to complete
    let find_toggles_start = perf.now();
    let find_toggles_doc = document.clone();
    let Found {found: toggles, ..} =
      wait_for(5000, move || {
      find_toggles_doc
        .query_selector_all(".toggle")
        .ok()
        .map(|list| -> Option<Vec<HtmlInputElement>> {
          if list.length() != 100 {
            trace!("list size: {}", list.length());
            return None;
          }

          let mut elements = vec![];
          for i in 0..list.length() {
            if let Some(element) = list.get(i) {
              elements.push(element.unchecked_into());
            } else {
              return None;
            }
          }
          Some(elements)
        })
        .flatten()
    })
    .await
    .map_err(|_| "todos could not be found to complete".to_string())?;
    trace!("  found complete toggles");

    for input in toggles.into_iter() {
      input.click();
    }
    self.benchmark.todos_completed = perf_since(find_toggles_start);
    trace!("  completed/toggled todos");

    // Find the destroy toggle
    // Some frameworks are weird and re-use elements so we can't simply iterate
    // over all the destroy toggles - instead we have to get the first destroy
    // toggle and delete it, confirm it and continue...
    //
    // First assert that our list is 100 elements
    let delete_doc = document.clone();
    let Found {..} =
      wait_for(
        1000,
        move || {
          delete_doc
            .query_selector_all(".destroy")
            .ok()
            .map(|list| {
              if list.length() == 100 {
                Some(())
              } else {
                trace!("len: {}", list.length());
                None
              }
            })
        }
      )
      .await
      .map_err(|_| "could not confirm destroy toggles exist".to_string())?;

    let delete_todos_start = perf.now();
    let start_destruction = perf.now();
    'destroy_todos: loop {
      let delete_doc = document.clone();

      let Found{found:may_node, ..} =
        wait_for(
          100,
          move || {
            delete_doc
              .query_selector(".destroy")
              .ok()
          }
        )
        .await
        .map_err(|_| "could not find todos to destroy".to_string())?;

      if let Some(node) = may_node {
        node.unchecked_ref::<HtmlElement>().click();
        wait(0).await;
        if perf.now() - start_destruction > 5000.0 {
          return Err("timed out during destroy todos".to_string());
        }
      } else {
        break 'destroy_todos;
      }
    }

    let delete_doc = document.clone();
    let Found {..} =
      wait_for(
        5000,
        move || {
          delete_doc
          .query_selector_all(".destroy")
          .ok()
          .map(|list| {
            if list.length() == 0 {
              Some(())
            } else {
              trace!("  there are {} destroy toggles left", list.length());
              None
            }
          }
          )
        .flatten()
        }
      )
      .await
      .map_err(|_| "could not destroy todos".to_string())?;
    self.benchmark.todos_deleted = perf_since(delete_todos_start);
    trace!("  confirmed destroyed todos");
    Ok(())
  }
}


/// Handles running the benchmarks for one framework step by step
pub struct BenchRunner {
  iframe: Option<HtmlIFrameElement>,
  bench_url: String,
  bench_name: String,
  bench_language: Option<String>,
  create_todo_method: CreateTodoMethod,
  step_runner: Option<StepRunner>,
}


impl BenchRunner {
  pub fn new() -> Self {
    BenchRunner {
      iframe: None,
      bench_url: "not a url".into(),
      bench_name: "unnamed benchmark".into(),
      bench_language: None,
      create_todo_method: CreateTodoMethod::Change,
      step_runner: None,
    }
  }
}


impl Component for BenchRunner {
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
      In::Iframe(iframe) => {
        self.iframe = Some(iframe.clone());
      }

      In::InitBench {
        url,
        create_todo_method,
        language,
        name,
      } => {
        self.create_todo_method = create_todo_method.clone();
        self.bench_url = url.clone();
        self.bench_name = name.clone();
        self.bench_language = language.clone();
      }

      In::Step => {
        trace!("step");
        let iframe = self.iframe.as_ref().expect("no iframe");

        let mut step_runner =
          self.step_runner.take().unwrap_or({
          let mut benchmark = Benchmark::new();
          benchmark.name = self.bench_name.clone();
          benchmark.language = self.bench_language.clone();
          StepRunner {
            iframe: iframe.clone(),
            create_todo_method: self.create_todo_method.clone(),
            benchmark,
          }
        });
        step_runner.create_todo_method = self.create_todo_method.clone();

        let tx = tx.clone();
        let url = self.bench_url.clone();
        sub.send_async(async move {
          tx.send(&Out::StepDisabled(true));

          let res = step_runner.execute_bench(&tx, url).await;

          if let Err(err) = res {
            error!("{}", err);
            step_runner.benchmark.failed_message = Some(err.clone());
          }

          In::BenchCompleted(step_runner)
        });
      }

      In::BenchCompleted(step_runner) => {
        trace!("bench completed");
        tx.send(&Out::StepDisabled(false));
        tx.send(&Out::Done(step_runner.benchmark.clone()));
      }
    }
  }

  fn view(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>,
  ) -> Gizmo<HtmlElement> {
    iframe()
      .class("todo-src embed-responsive-item")
      .rx_attribute("src", None, rx.branch_filter_map(|msg| msg.iframe_src()))
      .tx_post_build(tx.contra_map(|el: &HtmlElement| {
        In::Iframe(el.clone().dyn_into::<HtmlIFrameElement>().unwrap_throw())
      }))
  }
}
