use log::{trace, Level};
use mogwai::prelude::*;
use rand::{seq::SliceRandom, thread_rng};
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
    use wasm_bindgen_test::{wasm_bindgen_test_configure, *};

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
        let found_el = wait_for(2000, || document().get_element_by_id("my_div")).await;
        assert!(found_el.is_ok());
        let found_el = found_el.unwrap();
        assert!(found_el.elapsed >= 1000.0 && found_el.elapsed < 2000.0);
    }

    #[wasm_bindgen_test]
    async fn test_can_wait_for_all() {
        wait_and_build_div(1000, "my_div_a", "my_div");
        wait_and_build_div(1000, "my_div_b", "my_div");
        wait_and_build_div(1000, "my_div_c", "my_div");
        let found_el = wait_for(2000, || {
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
    RunNext,
    StepDisabled(bool),
    SuiteCompleted {
        benchmark: Benchmark,
        framework: FrameworkCard,
    },
    CompletionToggleInput(HtmlInputElement),
    ToggleAll,
}


pub struct App {
    cards: Vec<Gizmo<FrameworkCard>>,
    bench_runner: Gizmo<BenchRunner>,
    container: Option<HtmlElement>,
    benchmarks: Vec<Benchmark>,
    frameworks: Option<Vec<FrameworkCard>>,
    avg_times: u32,
    graph: Option<View<SvgsvgElement>>,
    toggle_all_input: Option<HtmlInputElement>,
}


impl App {
    pub fn new() -> Self {
        let cards = all_cards()
            .into_iter()
            .map(|card| Gizmo::from(card))
            .collect::<Vec<_>>();

        let bench_runner = Gizmo::from(BenchRunner::default());

        App {
            container: None,
            avg_times: 1,
            cards,
            bench_runner,
            benchmarks: vec![],
            frameworks: None,
            graph: None,
            toggle_all_input: None,
        }
    }

    fn find_framework_card_by_name(&self, name: &str) -> Option<&Gizmo<FrameworkCard>> {
        for gizmo in self.cards.iter() {
            let card_name = gizmo.with_state(|card| card.name.clone());
            if card_name == name {
                return Some(gizmo);
            }
        }
        None
    }

    /// Init the benchmark run
    fn init_run(&mut self) {
        // Causes the graph to be dropped (also from the DOM).
        self.graph = None;
        let container = self.container.as_ref().expect("no container");
        let _ = container.append_child(self.bench_runner.dom_ref());

        // Set all the cards to "ready"
        for card in self.cards.iter_mut() {
            card.update(&framework_card::In::ChangeState(FrameworkState::Ready));
        }

        // Gather all the frameworks we'll run
        let mut frameworks = vec![];
        for _ in 1..=self.avg_times {
            let mut frameworks_run = vec![];
            for gizmo in self.cards.iter() {
                let may_card = gizmo.with_state(|card| {
                    if card.is_enabled {
                        Some(card.clone())
                    } else {
                        None
                    }
                });
                if let Some(card) = may_card {
                    frameworks_run.push(card.clone());
                }
            }
            // Randomize the order
            let mut rng = thread_rng();
            frameworks_run.shuffle(&mut rng);
            frameworks.extend(frameworks_run);
        }
        self.frameworks = Some(frameworks);
    }

    /// Display the results
    fn display_results(&mut self) {
        let benchmarks = std::mem::replace(&mut self.benchmarks, vec![]);
        // Write the benchmarks to local storage if possible
        let _ = store::write_items(&benchmarks);
        // Graph them
        let graph = graph::graph_benchmarks(&benchmarks);
        // Remove the bench runner dom node
        let bench_runner_dom = self.bench_runner.dom_ref();
        bench_runner_dom
            .parent_node()
            .unwrap()
            .replace_child(&graph, bench_runner_dom)
            .unwrap();
        self.graph = Some(graph);

        trace!("done.");
    }
}


#[derive(Clone)]
pub enum Out {
    IframeSrc(String),
    RunningFramework {
        name: String,
        remaining: u32,
    },
    SetAvgTimesValue(String),
    StepDisabled(bool),
    RunDisabled(bool),
    SuiteCompleted {
        benchmark: Benchmark,
        framework: FrameworkCard,
    },
    SuiteFailed {
        benchmark: Benchmark,
        framework: FrameworkCard,
    },
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
                    bench_runner::Out::Done {
                        benchmark,
                        framework,
                    } => Some(In::SuiteCompleted {
                        framework: framework.clone(),
                        benchmark: benchmark.clone(),
                    }),
                    bench_runner::Out::StepDisabled(is_disabled) => {
                        Some(In::StepDisabled(*is_disabled))
                    }
                    _ => None,
                });
            }

            In::AvgOverTimesChange(event) => {
                let may_input = event
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
                        sub.send_async(async { In::ClickedRun });
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
                self.init_run();
                sub.send_async(async { In::RunNext });
                tx.send(&Out::RunDisabled(true));
            }

            In::RunNext => {
                trace!("run next");
                if let Some(mut frameworks) = self.frameworks.take() {
                    if let Some(next_framework) = frameworks.pop() {
                        if let Some(card_gizmo) =
                            self.find_framework_card_by_name(&next_framework.name)
                        {
                            card_gizmo
                                .update(&framework_card::In::ChangeState(FrameworkState::Running));
                        }
                        tx.send(&Out::RunningFramework{
                            name: next_framework.name.clone(),
                            remaining: frameworks.len() as u32,
                        });
                        self.frameworks = Some(frameworks);
                        self.bench_runner.update(&bench_runner::In::Run {
                            framework: next_framework,
                        });
                    } else {
                        self.frameworks = None;
                        self.display_results();
                    }
                }
            }

            In::StepDisabled(is_disabled) => {
                tx.send(&Out::StepDisabled(*is_disabled));
            }

            In::SuiteCompleted {
                benchmark,
                framework,
            } => {
                tx.send(&Out::RunDisabled(false));
                self.benchmarks.push(benchmark.clone());

                if let Some(card_gizmo) = self.find_framework_card_by_name(&framework.name) {
                    let card_state = if let Some(msg) = benchmark.failed_message.as_ref() {
                        FrameworkState::Erred(msg.clone())
                    } else {
                        FrameworkState::Done
                    };
                    card_gizmo.update(&framework_card::In::ChangeState(card_state));
                }

                sub.send_async(async { In::RunNext });
            }

            In::CompletionToggleInput(el) => {
                self.toggle_all_input = Some(el.clone());
            }

            In::ToggleAll => {
                let input = self.toggle_all_input.as_ref().unwrap_throw();
                let is_enabled = input.checked();
                for card in self.cards.iter_mut() {
                    card.update(&framework_card::In::IsEnabled(is_enabled));
                }
            }
        }
    }

    fn view(
        &self,
        tx: &Transmitter<Self::ModelMsg>,
        rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<HtmlElement> {
        let card_refs: Vec<HtmlElement> = self
            .cards
            .iter()
            .map(|gizmo| gizmo.dom_ref().clone())
            .collect();

        builder! {
            <div id="main" class="container-fluid">
                <nav class="navbar navbar-expand-lg navbar-light bg-light rounded-sm mt-2 mb-4">
                    <a href="https://github.com/schell/todo-mvc-bench">"schell's todo-mvc-bench"</a>
                    <ul class="navbar-nav ml-2 mr-auto">
                        <li class="nav-item mr-1">
                            <span>
                            {(
                                "",
                                rx.branch_filter_map(|msg| match msg {
                                    Out::RunningFramework{name, ..} => Some(name.clone()),
                                    _ => None,
                                })
                            )}
                            </span>
                        </li>
                        <li class="nav-item">
                            <span>
                            {(
                                "",
                                rx.branch_filter_map(|msg| match msg {
                                    Out::RunningFramework{remaining, ..} => {
                                        Some(format!("{} remaining", remaining))
                                    }
                                    _ => None,
                                })
                            )}
                            </span>
                        </li>
                    </ul>
                    <div class="input-group col-2">
                        <div class="input-group-prepend">
                            <span class="input-group-text">"avg over"</span>
                        </div>
                        <input
                            type="text"
                            class="form-control"
                            placeholder="1"
                            on:change = tx.contra_map(|event: &Event| {
                                In::AvgOverTimesChange(event.clone())
                            })
                            on:keyup = tx.contra_filter_map(|event: &Event| {
                                let event = event.dyn_ref::<KeyboardEvent>()?;
                                if event.key() == "Enter" {
                                    Some(In::AvgOverTimesChange(
                                        event.unchecked_ref::<Event>().clone(),
                                    ))
                                } else {
                                    None
                                }
                            })
                        />
                        <div class="input-group-append">
                            <button
                                id="run_button"
                                class="btn btn-primary"
                                on:click=tx.contra_map(|_| In::ClickedRun)
                                boolean:disabled=rx.branch_filter_map(|msg| match msg {
                                    Out::RunDisabled(disabled) => Some(*disabled),
                                    _ => None,
                                })>
                                "Run"
                            </button>
                        </div>
                    </div>
                </nav>
                <div class="container">
                    <div class="row embed-responsive embed-responsive-16by9 mb-4"
                        post:build=tx.contra_map(|el: &HtmlElement| In::Container(el.clone()))>
                    </div>
                    <div class="row mb-4 embed-responsive">
                        <table class="table table-bordered">
                            <thead>
                                <tr>
                                    <th scope="col">
                                        <input
                                            type="checkbox"
                                            style="cursor: pointer;"
                                            post:build=tx.contra_map(
                                                |el: &HtmlInputElement| {
                                                    In::CompletionToggleInput(el.clone())
                                                },
                                            )
                                            on:change=tx.contra_map(|_| In::ToggleAll)
                                        />
                                    </th>
                                    <th scope="col">"Frameworks"</th>
                                    <th scope="col">"Version"</th>
                                    <th scope="col">"Language"</th>
                                    <th scope="col">"vDOM"</th>
                                    <th scope="col">"Size"</th>
                                    <th scope="col">"Score"</th>
                                    <th scope="col">"Note"</th>
                                </tr>
                            </thead>
                            <tbody post:build=tx.contra_filter_map(move |el:&HtmlElement| {
                                for card in card_refs.iter() {
                                    el.append_child(card).expect("could not add card");
                                }
                                None
                            })>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        }
    }
}

#[wasm_bindgen]
pub fn bench() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let app = Gizmo::from(App::new());
    app.update(&In::Startup);
    app.run()
}
