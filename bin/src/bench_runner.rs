use log::{error, trace};
use mogwai::prelude::*;
use serde::{Deserialize, Serialize};
use web_sys::{Document, HtmlIFrameElement};

use todo_mvc_bench_lib::{
    async_event::{wait_for_event_on, EventResult},
    find::Found,
    wait, wait_for,
};

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
pub enum In {
    Iframe(HtmlIFrameElement),
    Run{ framework: FrameworkCard },
    BenchCompleted(StepRunner),
}

#[derive(Clone)]
pub enum Out {
    IframeSrc(String),
    StepDisabled(bool),
    Done {
        benchmark: Benchmark,
        framework: FrameworkCard
    },
}

impl Out {
    fn iframe_src(&self) -> Option<String> {
        match self {
            Out::IframeSrc(src) => Some(src.clone()),
            _ => None,
        }
    }
}

/// Handles running one step
#[derive(Clone)]
pub struct StepRunner {
    iframe: HtmlIFrameElement,
    framework: FrameworkCard,
    benchmark: Benchmark,
}

impl StepRunner {
    async fn load_step(
        &self,
        tx: &Transmitter<Out>,
        src: String,
        perf_now: impl Fn() -> f64,
    ) -> Result<BenchmarkStep, String> {
        let mut step = BenchmarkStep {
            name: "initial load".to_string(),
            start: perf_now(),
            end: None,
            cycles: None,
        };
        tx.send(&Out::IframeSrc(src.clone()));
        let _res: EventResult = wait_for_event_on("load", &self.iframe).await;
        step.end = Some(perf_now());
        Ok(step)
    }

    async fn find_todo_input(
        &self,
        doc: Document,
        perf_now: impl Fn() -> f64,
    ) -> Result<(HtmlInputElement, BenchmarkStep), String> {
        let mut await_todo_step = BenchmarkStep {
            name: "await todo input".to_string(),
            start: perf_now(),
            end: None,
            cycles: None,
        };
        let Found { found: input, .. } = wait_for(5000, move || {
            doc.query_selector("#new-todo")
                .ok()
                .flatten()
                .or(doc.query_selector(".new-todo").ok().flatten())
        })
        .await
        .map_err(|_| "todo input not found".to_string())?;
        let todo_input: HtmlInputElement = input.unchecked_into::<HtmlInputElement>();
        await_todo_step.end = Some(perf_now());
        Ok((todo_input, await_todo_step))
    }

    async fn wait_todo_focus(
        &self,
        input: &HtmlInputElement,
        perf_now: impl Fn() -> f64,
    ) -> Result<BenchmarkStep, String> {
        let mut await_focus_step = BenchmarkStep {
            name: "await todo focus".to_string(),
            start: perf_now(),
            end: None,
            cycles: None,
        };
        let _ = wait_for_event_on("focus", input).await;
        await_focus_step.end = Some(perf_now());
        Ok(await_focus_step)
    }

    async fn create_todos(
        &self,
        doc: Document,
        input: &HtmlInputElement,
        perf_now: impl Fn() -> f64,
    ) -> Result<BenchmarkStep, String> {
        let len = doc
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
        let created = new_shared::<u32, u32>(0);
        'create: loop {
            let doc = doc.clone();
            let len = doc
                .query_selector_all(".toggle")
                .map_err(|_| "could not query DOM with selector")
                .map(|list| list.length())?;

            if len > 100 {
                return Err("created too many todos".into());
            } else if len == 100 {
                break 'create;
            }

            input.focus().expect("could not focus input");
            let value = format!("Something to do {}", len);
            input.set_value(&value);
            self.framework.create_todo_method.dispatch_events(&doc, &input);

            let created = created.clone();
            let _ = wait_for::<(), _>(1000, move || {
                let new_length = doc
                    .query_selector_all(".toggle")
                    .ok()
                    .map(|list| list.length());
                if let Some(new_len) = new_length {
                    if len + 1 == new_len {
                        *created.borrow_mut() = new_len;
                        return Some(())
                    }
                }
                None
            })
            .await
            .map_err(|_| "timed out waiting for todo creation".to_string())?;
        }
        create_todos_step.end = Some(perf_now());
        Ok(create_todos_step)
    }

    async fn complete_todos(
        &self,
        doc: Document,
        perf_now: impl Fn() -> f64,
    ) -> Result<BenchmarkStep, String> {
        let mut complete_todos_step = BenchmarkStep {
            name: "complete todos".to_string(),
            start: perf_now(),
            end: None,
            cycles: None,
        };
        let Found { found: toggles, .. } = wait_for(5000, move || {
            doc.query_selector_all(".toggle")
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
        complete_todos_step.end = Some(perf_now());
        Ok(complete_todos_step)
    }

    async fn delete_todos(
        &self,
        doc: Document,
        perf_now: impl Fn() -> f64,
    ) -> Result<BenchmarkStep, String> {
        // Find the destroy toggle
        // Some frameworks are weird and re-use elements so we can't simply iterate
        // over all the destroy toggles - instead we have to get the first destroy
        // toggle and delete it, confirm it and continue...
        //
        // First assert that our list is 100 elements
        let delete_doc = doc.clone();
        let Found { .. } = wait_for(1000, move || {
            delete_doc.query_selector_all(".destroy").ok().map(|list| {
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
        'destroy_todos: loop {
            let delete_doc = doc.clone();

            let Found {
                found: may_node, ..
            } = wait_for(100, move || delete_doc.query_selector(".destroy").ok())
                .await
                .map_err(|_| "could not find todos to destroy".to_string())?;

            if let Some(node) = may_node {
                node.unchecked_ref::<HtmlElement>().click();
                wait(0).await;
                if perf_now() - start_destruction > 5000.0 {
                    return Err("timed out during destroy todos".to_string());
                }
            } else {
                break 'destroy_todos;
            }
        }
        let delete_doc = doc.clone();
        let Found { .. } = wait_for(5000, move || {
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
                })
                .flatten()
        })
        .await
        .map_err(|_| "could not destroy todos".to_string())?;
        delete_todos_step.end = Some(perf_now());
        Ok(delete_todos_step)
    }

    async fn execute_bench(&mut self, tx: &Transmitter<Out>, src: String) -> Result<(), String> {
        let perf = window().performance().ok_or("no performance")?;
        let bench_start = perf.now();
        let perf_now = move || perf.now() - bench_start;

        // Load the iframe source
        trace!("{} waiting for iframe load complete", src);
        self.benchmark
            .steps
            .push(self.load_step(tx, src, perf_now.clone()).await?);
        trace!("  load complete");

        let document = self
            .iframe
            .content_document()
            .expect("no iframe content_document");

        trace!("finding todo input");
        let (input, step) = self
            .find_todo_input(document.clone(), perf_now.clone())
            .await?;
        self.benchmark.steps.push(step);
        trace!("  found todo input");

        if self.framework.wait_for_input_focus {
            trace!("waiting for todo focus");
            self.benchmark
                .steps
                .push(self.wait_todo_focus(&input, perf_now.clone()).await?);
            trace!("  todo is focused");
        }

        trace!("creating todos");
        self.benchmark.steps.push(
            self.create_todos(document.clone(), &input, perf_now.clone())
                .await?,
        );
        trace!("  created todos");

        trace!("completing todos");
        self.benchmark.steps.push(
            self.complete_todos(document.clone(), perf_now.clone())
                .await?,
        );
        trace!("  completed/toggled todos");

        trace!("deleting todos");
        self.benchmark.steps.push(
            self.delete_todos(document.clone(), perf_now.clone())
                .await?,
        );
        trace!("  confirmed destroyed todos");
        Ok(())
    }
}

/// Handles running the benchmarks for one framework step by step
pub struct BenchRunner {
    iframe: Option<HtmlIFrameElement>,
    step_runner: Option<StepRunner>,
}


impl Default for BenchRunner {
    fn default() -> Self {
        BenchRunner {
            iframe: None,
            step_runner: None
        }
    }
}


impl Component for BenchRunner {
    type ModelMsg = In;
    type ViewMsg = Out;
    type DomNode = HtmlIFrameElement;

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

            In::Run { framework } => {
                trace!("step");
                let iframe = self.iframe.as_ref().expect("no iframe");

                let mut step_runner = self.step_runner.take().unwrap_or({
                    let mut benchmark = Benchmark::new();
                    benchmark.name = framework.name.clone();
                    benchmark.language = framework.framework_attribute("language").clone();
                    StepRunner {
                        iframe: iframe.clone(),
                        benchmark,
                        framework: framework.clone()
                    }
                });

                let tx = tx.clone();
                let url = framework.url.clone();
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
                tx.send(&Out::Done{
                    benchmark: step_runner.benchmark.clone(),
                    framework: step_runner.framework.clone()
                });
            }
        }
    }

    fn view(
        &self,
        tx: &Transmitter<Self::ModelMsg>,
        rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<HtmlIFrameElement> {
        builder! {
            <iframe
                class = "todo-src embed-responsive-item"
                src = rx.branch_filter_map(|msg| msg.iframe_src())
                post:build = tx.contra_map(|el: &HtmlIFrameElement| In::Iframe(el.clone()))>
            </iframe>
        }
    }
}
