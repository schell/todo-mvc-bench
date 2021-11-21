use log::{error, trace};
use mogwai::{
    event::{event_stream, event_stream_with},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use web_sys::{Document, HtmlIFrameElement};

use todo_mvc_bench_lib::{wait_for, wait_until_next_for, wait_while, Found};

use crate::framework_card::CreateTodoMethod;

use super::framework_card::FrameworkCard;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkStep {
    pub name: String,
    pub start: f64,
    pub end: Option<f64>,
    pub cycles: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Benchmark {
    pub name: String,
    pub steps: Vec<BenchmarkStep>,
    pub failed_message: Option<String>,
    pub language: Option<String>,
}

impl Benchmark {
    pub fn new() -> Self {
        Benchmark {
            name: "unnamed".into(),
            steps: vec![],
            failed_message: None,
            language: None,
        }
    }

    pub fn total(&self) -> Option<f64> {
        self.steps.iter().fold(Some(0.0), |may_sum, step| {
            let sum = may_sum?;
            let end = step.end?;
            Some(sum + (end - step.start))
        })
    }
}

#[derive(Clone)]
pub struct Run {
    framework: FrameworkCard,
    reply: broadcast::Sender<Benchmark>,
}

#[derive(Clone)]
pub enum ViewMsg {
    IframeSrc(String),
    StepDisabled(bool),
}

struct Done {
    benchmark: Benchmark,
    framework: FrameworkCard,
}

impl ViewMsg {
    fn iframe_src(&self) -> Option<String> {
        match self {
            ViewMsg::IframeSrc(src) => Some(src.clone()),
            _ => None,
        }
    }
}

async fn load_step(
    iframe: Dom,
    tx: broadcast::Sender<ViewMsg>,
    src: String,
    perf_now: impl Fn() -> f64,
) -> Result<BenchmarkStep, String> {
    let mut loads = event_stream_with(
        "load",
        &iframe
            .clone_as::<EventTarget>()
            .ok_or_else(|| "iframe is not an event target".to_string())?,
        |ev| Dom::try_from(JsValue::from(ev)).unwrap(),
    );
    let mut step = BenchmarkStep {
        name: "initial load".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };

    tx.broadcast(ViewMsg::IframeSrc(src.clone())).await.unwrap();
    let event = loads.next().await.unwrap();
    step.end = Some(perf_now());
    Ok(step)
}

async fn find_todo_input(
    document: Dom,
    perf_now: impl Fn() -> f64,
) -> Result<(Dom, BenchmarkStep), String> {
    let mut await_todo_step = BenchmarkStep {
        name: "await todo input".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };
    let Found {
        found: todo_input, ..
    } = wait_for(5.0, move || {
        let doc = document.clone_as::<Document>().unwrap();
        let el = if let Some(new_todo) = doc.query_selector("#new-todo").ok()? {
            trace!("found #new-todo");
            Some(new_todo)
        } else {
            if let Some(new_todo) = doc.query_selector(".new-todo").ok()? {
                trace!("found .new-todo");
                Some(new_todo)
            } else {
                trace!("can't find #new-todo nor .new-todo");
                None
            }
        }?;

        Some(Dom::try_from(JsValue::from(el)).unwrap())
    })
    .await
    .map_err(|_| "todo input not found".to_string())?;
    await_todo_step.end = Some(perf_now());
    Ok((todo_input, await_todo_step))
}

async fn wait_todo_focus(input: Dom, perf_now: impl Fn() -> f64) -> Result<BenchmarkStep, String> {
    let focus_events = event_stream_with(
        "focus",
        &input.clone_as::<web_sys::EventTarget>().unwrap(),
        |ev| Dom::try_from(JsValue::from(ev)).unwrap(),
    );
    let mut await_focus_step = BenchmarkStep {
        name: "await todo focus".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };
    let _ = wait_until_next_for(5.0, focus_events)
        .await
        .map_err(|e| format!("timed out waiting for focus for {} seconds", e))?;
    await_focus_step.end = Some(perf_now());
    Ok(await_focus_step)
}

async fn create_todos(
    document: Dom,
    input: Dom,
    create_todo_method: CreateTodoMethod,
    perf_now: impl Fn() -> f64,
) -> Result<BenchmarkStep, String> {
    let len = document
        .clone_as::<Document>()
        .unwrap()
        .query_selector_all(".toggle")
        .map_err(|_| "could not query DOM with selector")
        .map(|list| list.length())?;
    if len > 0 {
        return Err("pre-existing todos".into());
    }

    let mut create_todos_step = BenchmarkStep {
        name: "create todos".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };
    let mut created: u32 = 0;
    while created < 100 {
        let len = document
            .clone_as::<Document>()
            .unwrap()
            .query_selector_all(".toggle")
            .map_err(|_| "could not query DOM with selector")
            .map(|list| list.length())?;

        if len > 100 {
            return Err("created too many todos".into());
        }

        let value = format!("Something to do {}", len);
        let _ = input.visit_as(
            |i: &web_sys::HtmlInputElement| {
                i.focus().expect("could not focus input");
                i.set_value(&value);
            },
            |_| {},
        );
        create_todo_method.dispatch_events(
            &document.clone_as::<Document>().unwrap(),
            input.clone_as::<web_sys::HtmlInputElement>().unwrap(),
        );

        let document = document.clone();
        let _ = wait_for(1.0, move || {
            let new_length = document
                .clone_as::<Document>()
                .unwrap()
                .query_selector_all(".toggle")
                .ok()
                .map(|list| list.length());
            if let Some(new_len) = new_length {
                if len + 1 == new_len {
                    return Some(());
                }
            }
            None
        })
        .await
        .map_err(|e| format!("timed out waiting for todo creation for {} seconds", e))?;
        created += 1;
    }
    create_todos_step.end = Some(perf_now());
    Ok(create_todos_step)
}

async fn complete_todos(
    document: Dom,
    perf_now: impl Fn() -> f64,
) -> Result<BenchmarkStep, String> {
    let mut complete_todos_step = BenchmarkStep {
        name: "complete todos".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };
    let Found { found: toggles, .. } = wait_for(5.0, move || -> Option<Vec<Dom>> {
        document
            .clone_as::<Document>()
            .unwrap()
            .query_selector_all(".toggle")
            .ok()
            .map(|list| -> Option<Vec<Dom>> {
                if list.length() != 100 {
                    trace!("list size: {}", list.length());
                    return None;
                }

                let mut elements = vec![];
                for i in 0..list.length() {
                    if let Some(element) = list.get(i) {
                        let dom = Dom::try_from(JsValue::from(element)).unwrap();
                        elements.push(dom);
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
        input
            .clone_as::<web_sys::HtmlInputElement>()
            .unwrap()
            .click();
    }
    complete_todos_step.end = Some(perf_now());
    Ok(complete_todos_step)
}

async fn delete_todos(document: Dom, perf_now: impl Fn() -> f64) -> Result<BenchmarkStep, String> {
    // Find the destroy toggle
    // Some frameworks are weird and re-use elements so we can't simply iterate
    // over all the destroy toggles - instead we have to get the first destroy
    // toggle and delete it, confirm it and continue...
    //
    // First assert that our list is 100 elements
    let doc = document.clone();
    let Found { .. } = wait_for(1.0, move || {
        doc.clone_as::<Document>()
            .unwrap()
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
    })
    .await
    .map_err(|_| "could not confirm destroy toggles exist".to_string())?;

    let mut delete_todos_step = BenchmarkStep {
        name: "delete todos".to_string(),
        start: perf_now(),
        end: None,
        cycles: None,
    };
    let start_destruction = perf_now();
    let mut deletions_remaining = 100;
    'destroy_todos: loop {
        trace!("  {}", deletions_remaining);
        let el_dom = {
            let list = document
                .clone_as::<Document>()
                .unwrap()
                .query_selector_all(".destroy")
                .map_err(|_| "could not get .destroy nodelist".to_string())?;
            if list.length() != deletions_remaining {
                // We are still waiting for the previous one to have disappeared
                return Err(format!(
                    "unexpected number of todos: {}",
                    deletions_remaining
                ));
            }

            let el: HtmlElement = list.get(0)
                .ok_or_else(|| "no destroy button to click".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "destroy button is not an HtmlElement".to_string())?;
            el.click();
            Dom::try_from(JsValue::from(el)).unwrap()
        };

        deletions_remaining -= 1;

        let doc = document.clone();
        let Found { .. } = wait_while(5.0, move || {
            let list = doc.clone_as::<Document>()
                .unwrap()
                .query_selector_all(".destroy")
                .unwrap();
            list.length() != deletions_remaining
        })
        .await
        .map_err(|elapsed| format!("couldn't confirm todo deleted after {} seconds", elapsed))?;

        if deletions_remaining == 0 {
            break 'destroy_todos;
        }
    }
    {
        let num_destroy_toggles = document
            .clone_as::<Document>()
            .unwrap()
            .query_selector_all(".destroy")
            .map(|list: web_sys::NodeList| list.length())
            .map_err(|_| "couldn't get destroy toggle nodelist to confirm deletion".to_string())?;

        if num_destroy_toggles > 0 {
            return Err(format!("there are {} remaining todos", num_destroy_toggles));
        }
    }
    delete_todos_step.end = Some(perf_now());
    Ok(delete_todos_step)
}

async fn execute_bench(
    framework: FrameworkCard,
    iframe: Dom,
    tx: broadcast::Sender<ViewMsg>,
    src: String,
) -> Result<Vec<BenchmarkStep>, String> {
    let mut steps = vec![];
    let bench_start = mogwai::utils::window()
        .performance()
        .ok_or_else(|| "no performance object".to_string())?
        .now();
    let perf_now = move || mogwai::utils::window().performance().unwrap().now() - bench_start;

    // Load the iframe source
    trace!("{} waiting for iframe load complete", src);
    let some_steps = load_step(iframe.clone(), tx, src, perf_now.clone()).await?;
    steps.push(some_steps);
    trace!("  load complete");

    let document = iframe
        .visit_as(
            |iframe: &HtmlIFrameElement| {
                let val = JsValue::from(iframe.content_document().unwrap());
                Dom::try_from(val).unwrap()
            },
            |_| panic!("wasm only"),
        )
        .expect("no iframe content_document");

    trace!("finding todo input");
    let (input, step) = find_todo_input(document.clone(), perf_now.clone()).await?;
    steps.push(step);
    trace!("  found todo input");

    if framework.wait_for_input_focus {
        trace!("waiting for todo focus");
        steps.push(wait_todo_focus(input.clone(), perf_now.clone()).await?);
        trace!("  todo is focused");
    }

    trace!("creating todos");
    steps.push(
        create_todos(
            document.clone(),
            input.clone(),
            framework.create_todo_method,
            perf_now.clone(),
        )
        .await?,
    );
    trace!("  created todos");

    trace!("completing todos");
    steps.push(complete_todos(document.clone(), perf_now.clone()).await?);
    trace!("  completed/toggled todos");

    trace!("deleting todos");
    steps.push(delete_todos(document.clone(), perf_now.clone()).await?);
    trace!("  confirmed destroyed todos");
    Ok(steps)
}

/// Handles running the benchmarks for one framework step by step
async fn bench_runner_logic(
    mut rx_logic: broadcast::Receiver<Run>,
    tx: broadcast::Sender<ViewMsg>,
    rx_iframe: mpmc::Receiver<Dom>,
) {
    let iframe = rx_iframe.recv().await.unwrap();
    loop {
        match rx_logic.next().await {
            Some(Run { framework, reply }) => {
                trace!("running {}", framework.name);

                let mut benchmark = Benchmark::new();
                benchmark.name = framework.name.clone();
                benchmark.language = framework.framework_attribute("language").clone();

                let url = framework.url.clone();
                tx.broadcast(ViewMsg::StepDisabled(true)).await.unwrap();

                let res = execute_bench(framework.clone(), iframe.clone(), tx.clone(), url).await;
                match res {
                    Ok(steps) => {
                        benchmark.steps.extend(steps);
                    }
                    Err(err) => {
                        error!("{}", err);
                        benchmark.failed_message = Some(err.clone());
                    }
                }

                trace!("bench completed");
                tx.broadcast(ViewMsg::StepDisabled(false)).await.unwrap();
                if let Err(e) = reply.broadcast(benchmark).await {
                    log::warn!(
                        "cannot send complete benchmark (probably got canceled): {}",
                        e
                    );
                }
            }
            None => break,
        }
    }
}

fn view(
    tx_iframe: mpmc::Sender<Dom>,
    tx: broadcast::Sender<Run>,
    rx: broadcast::Receiver<ViewMsg>,
) -> ViewBuilder<Dom> {
    builder! {
        <iframe
         class="todo-src embed-responsive-item"
         src=rx.clone().filter_map(|msg| async move {msg.iframe_src()})
         post:build=move |dom: &mut Dom| tx_iframe.try_send(dom.clone()).unwrap()>
        </iframe>
    }
}

pub struct BenchRunnerFacade {
    tx_logic: broadcast::Sender<Run>,
}

impl BenchRunnerFacade {
    pub fn create() -> (Self, Component<Dom>) {
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let (tx_iframe, rx_iframe) = mpmc::bounded(1);
        let component = Component::from(view(tx_iframe, tx_logic.clone(), rx_view))
            .with_logic(bench_runner_logic(rx_logic, tx_view, rx_iframe));
        (BenchRunnerFacade { tx_logic }, component)
    }

    pub async fn run(&self, framework: FrameworkCard) -> Benchmark {
        let (tx, mut rx) = broadcast::bounded(1);
        self.tx_logic
            .broadcast(Run {
                framework,
                reply: tx,
            })
            .await
            .unwrap();
        rx.next().await.unwrap()
    }
}
