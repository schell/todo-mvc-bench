use log::{trace, Level};
use mogwai::{lock::RwLock, prelude::*};
use rand::{seq::SliceRandom, thread_rng};
use std::{collections::HashMap, panic, sync::Arc};
use todo_mvc_bench_lib::{wait_for, wait_while};
use wasm_bindgen::prelude::*;
use web_sys::{HtmlInputElement, KeyboardEvent, SvgsvgElement};

mod bench_runner;
use bench_runner::{BenchRunnerFacade, Benchmark};

mod framework_card;
use framework_card::{all_cards, FrameworkCard, FrameworkFacade, FrameworkState};

mod graph;
mod store;

//#[cfg(test)]
//mod bench_tests {
//    extern crate wasm_bindgen_test;
//
//    use mogwai::prelude::*;
//    use wasm_bindgen::UnwrapThrowExt;
//    use wasm_bindgen_test::{wasm_bindgen_test_configure, *};
//
//    use todo_mvc_bench_lib::wait_for;
//
//    wasm_bindgen_test_configure!(run_in_browser);
//
//    async fn wait_and_build_div(seconds: f64, id: &str, class: &str) {
//        let id: String = id.into();
//        let class: String = class.into();
//        let _ = mogwai::time::wait_approx(seconds * 1000.0).await;
//        let view = view! {
//            <div id=id class=class></div>
//        };
//        view.run().unwrap_throw();
//    }
//
//    #[wasm_bindgen_test]
//    async fn test_can_wait_for_one() {
//        wait_and_build_div(1.0, "my_div", "");
//        let found_el = wait_for(2.0, || {
//            mogwai::utils::document().get_element_by_id("my_div")
//        })
//        .await;
//        assert!(found_el.is_ok());
//        let found_el = found_el.unwrap();
//        assert!(found_el.elapsed >= 1.0 && found_el.elapsed < 2.0);
//    }
//
//    #[wasm_bindgen_test]
//    async fn test_can_wait_for_all() {
//        wait_and_build_div(1.0, "my_div_a", "my_div");
//        wait_and_build_div(1.0, "my_div_b", "my_div");
//        wait_and_build_div(1.0, "my_div_c", "my_div");
//        let found_el = wait_for(2.0, || {
//            mogwai::utils::document()
//                .query_selector_all(".my_div")
//                .ok()
//                .map(|list| if list.length() > 0 { Some(list) } else { None })
//                .flatten()
//        })
//        .await;
//        assert!(found_el.is_ok());
//        let found_el = found_el.unwrap();
//        assert!(found_el.elapsed >= 1.0 && found_el.elapsed < 2.0);
//        assert!(found_el.found.length() == 3)
//    }
//}

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Clone)]
pub enum In {
    // Number of repititions to run each test, or reset
    // to the last saved value.
    AvgOverTimesChange {
        changed_times: Option<u32>,
        hit_enter: bool,
    },
    SoloFramework(String),
    ClickedRun,
    ToggleAll,
}

impl In {
    fn from_repititions_change_event(event: web_sys::Event) -> In {
        let may_input = event
            .target()
            .map(|t| t.clone().unchecked_into::<HtmlInputElement>());

        let changed_times = may_input
            .map(|input| input.value().trim().parse::<u32>().ok())
            .flatten();

        let hit_enter = if let Some(event) = event.dyn_ref::<KeyboardEvent>() {
            event.key() == "Enter"
        } else {
            false
        };

        In::AvgOverTimesChange {
            changed_times,
            hit_enter,
        }
    }
}

pub struct App {
    cards: HashMap<String, FrameworkFacade>,
    //benchmarks: Vec<Benchmark>,
    avg_times: u32,
}

impl App {
    //pub fn new() -> Self {
    //    let cards = all_cards()
    //        .into_iter()
    //        .fold(ListPatchModel::new(), |mut model, card| {
    //            model.list_patch_push(Arc::new(RwLock::new(card)));
    //            model
    //        });

    //    App {
    //        avg_times: 1,
    //        //            cards,
    //        //            benchmarks: vec![],
    //        frameworks: None,
    //    }
    //}
}

#[derive(Clone)]
pub enum Out {
    IframeSrc(String),
    RunningFramework { name: String, remaining: u32 },
    SetAvgTimesValue(String),
    RunDisabled(bool),
}

async fn app_logic(
    mut app: App,
    tx_logic: broadcast::Sender<In>,
    mut rx_logic: broadcast::Receiver<In>,
    mut rx_cancel: broadcast::Receiver<()>,
    tx_view: broadcast::Sender<Out>,
    tx_container: mpmc::Receiver<Dom>,
    tx_input: mpmc::Receiver<Dom>,
) {
    log::trace!("app logic startup");
    let toggle_all_input = tx_input.recv().await.unwrap();
    let container_dom = tx_container.recv().await.unwrap();

    // now that we have the test and results container, we can try to read
    // any previous benchmarks and show them here.
    if let Ok(benchmarks) = store::read_benchmarks() {
        let graph = Component::from(graph::graph_benchmarks(&benchmarks)).build().unwrap();
        container_dom.patch_children(ListPatch::push(graph.into_inner())).unwrap();
    }

    while let Some(msg) = rx_logic.next().await {
        match msg {
            In::AvgOverTimesChange {
                changed_times,
                hit_enter,
            } => {
                if let Some(new_times) = changed_times {
                    app.avg_times = new_times;
                } else {
                    let times = format!("{}", app.avg_times);
                    tx_view
                        .broadcast(Out::SetAvgTimesValue(times))
                        .await
                        .unwrap();
                }

                if hit_enter {
                    tx_logic.broadcast(In::ClickedRun).await.unwrap();
                }
            }

            In::SoloFramework(name) => {
                for facade in app.cards.values() {
                    let card = facade.get_card().await;
                    if card.name == name && !card.is_enabled {
                        facade.set_enabled(true).await;
                        break;
                    }
                }
            }

            In::ClickedRun => {
                trace!("starting run");
                // Causes the graph to be dropped from the DOM
                let (bench_runner_facade, bench_runner_component) = BenchRunnerFacade::create();
                let bench_runner_view = bench_runner_component.build().unwrap();
                container_dom
                    .patch_children(ListPatch::splice(
                        ..,
                        std::iter::once(bench_runner_view.into_inner()),
                    ))
                    .unwrap();

                // Set all the cards to "ready"
                for card in app.cards.values() {
                    card.set_state(FrameworkState::Ready).await;
                }

                // Gather all the frameworks we'll run
                let mut frameworks = vec![];
                for _ in 1..=app.avg_times {
                    let mut frameworks_run = vec![];
                    for facade in app.cards.values() {
                        let card: FrameworkCard = facade.get_card().await;
                        if card.is_enabled {
                            frameworks_run.push(card);
                        }
                    }
                    // Randomize the order of that run
                    let mut rng = thread_rng();
                    frameworks_run.shuffle(&mut rng);
                    frameworks.extend(frameworks_run);
                }

                trace!("running frameworks");
                let mut benchmarks = vec![];
                tx_view.broadcast(Out::RunDisabled(true)).await.unwrap();
                'bench_run: while let Some(next_framework) = frameworks.pop() {
                    if let Some(facade) = app.cards.get(&next_framework.name) {
                        facade.set_state(FrameworkState::Running).await;
                    }
                    tx_view
                        .broadcast(Out::RunningFramework {
                            name: next_framework.name.clone(),
                            remaining: frameworks.len() as u32,
                        })
                        .await
                        .unwrap();

                    let complete = bench_runner_facade.run(next_framework.clone()).fuse();
                    pin_mut!(complete);
                    let cancel = rx_cancel.next().fuse();
                    pin_mut!(cancel);

                    futures::select! {
                        benchmark = complete => {
                            if let Some(msg) = benchmark.failed_message.as_ref() {
                                if let Some(facade) = app.cards.get(&next_framework.name) {
                                    facade
                                        .set_state(FrameworkState::Erred(msg.to_string()))
                                        .await;
                                }
                            }
                            benchmarks.push(benchmark);
                        },
                        _ = cancel => {
                            log::warn!("canceled benchmark run");
                            break 'bench_run;
                        }
                    };
                }

                //// Write the benchmarks to local storage if possible
                let _ = store::write_items(&benchmarks);
                //// Graph them
                let graph = Component::from(graph::graph_benchmarks(&benchmarks));
                trace!("created the graph");
                let graph = graph
                    .build()
                    .unwrap_or_else(|e| panic!("couldn't create the graph: {}", e))
                    .into_inner();
                trace!("built the graph");
                // Remove the bench runner dom node and add the graph
                container_dom
                    .patch_children(ListPatch::splice(.., std::iter::once(graph)))
                    .unwrap();

                trace!("done.");
                tx_view.broadcast(Out::RunDisabled(false)).await.unwrap();
            }

            In::ToggleAll => {
                let is_enabled = toggle_all_input
                    .visit_as(|input: &HtmlInputElement| input.checked(), |_| false)
                    .unwrap_or(false);
                for facade in app.cards.values() {
                    facade.set_enabled(is_enabled).await;
                }
            }
        }
    }
}

fn app_view(
    app: &App,
    tx: broadcast::Sender<In>,
    tx_cancel: broadcast::Sender<()>,
    rx: broadcast::Receiver<Out>,

    tx_container: mpmc::Sender<Dom>,
    tx_input: mpmc::Sender<Dom>,

    cards: Vec<Component<Dom>>,
) -> ViewBuilder<Dom> {
    builder! {
        <div id="main" class="container-fluid">
            <nav class="navbar navbar-expand-lg navbar-light bg-light rounded-sm mt-2 mb-4">
                <a href="https://github.com/schell/todo-mvc-bench">"schell's todo-mvc-bench"</a>
                <ul class="navbar-nav ml-2 mr-auto">
                    <li class="nav-item mr-1">
                        <span>
                        {(
                            "",
                            rx.clone().filter_map(|msg| async move {
                                match msg {
                                    Out::RunningFramework{name, ..} => Some(name.clone()),
                                    _ => None,
                                }
                            })
                        )}
                        </span>
                    </li>
                    <li class="nav-item">
                        <span>
                        {(
                            "",
                            rx.clone().filter_map(|msg| async move {
                                match msg {
                                    Out::RunningFramework{remaining, ..} => {
                                        Some(format!("{} remaining", remaining))
                                    }
                                    _ => None,
                                }
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
                        on:change = tx.sink().contra_map(|event: Event| In::from_repititions_change_event(event))
                        on:keyup = tx.sink().contra_filter_map(|event: web_sys::Event| {
                            let key_event = event.dyn_ref::<KeyboardEvent>()?;
                            if key_event.key() == "Enter" {
                                Some(In::from_repititions_change_event(event))
                            } else {
                                None
                            }
                        })
                    />
                    <div class="input-group-append">
                        <button
                         id="run_button"
                         class="btn btn-primary"
                         on:click=tx.sink().contra_map(|_| In::ClickedRun)
                         boolean:disabled=rx.clone().filter_map(|msg| async move {
                             match msg {
                                 Out::RunDisabled(disabled) => Some(disabled),
                                 _ => None,
                             }
                         })>
                            "Run"
                        </button>

                        <button
                         id="cancel_button"
                         class="btn btn-warning"
                         style:cursor="pointer"
                         on:click=tx_cancel.sink().contra_map(|_| ())
                         boolean:disabled=(true, rx.clone().filter_map(|msg| async move {
                             match msg {
                                 Out::RunDisabled(disabled) => Some(!disabled),
                                 _ => None,
                             }
                         }))>
                            "Cancel"
                        </button>
                    </div>
                </div>
            </nav>
            <div class="container">
                <div class="row embed-responsive embed-responsive-16by9 mb-4"
                    post:build = move |dom: &mut Dom| tx_container.try_send(dom.clone()).unwrap()>
                </div>
                <div class="row mb-4 embed-responsive">
                    <table class="table table-bordered">
                        <thead>
                            <tr>
                                <th scope="col">
                                    <input
                                        type="checkbox"
                                        style="cursor: pointer;"
                                        post:build = move |dom: &mut Dom| tx_input.try_send(dom.clone()).unwrap()
                                        on:change=tx.sink().contra_map(|_| In::ToggleAll)
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
                        <tbody>
                            {cards}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

pub fn app_component() -> Component<Dom> {
    let (card_facades, card_components): (Vec<(String, _)>, Vec<_>) = framework_card::all_cards()
        .into_iter()
        .map(|card| {
            let name = card.name.clone();
            let (facade, component) = FrameworkFacade::create(card);
            ((name, facade), component)
        })
        .unzip();
    let app = App {
        cards: card_facades.into_iter().collect::<HashMap<_, _>>(),
        avg_times: 1,
    };
    let (tx_logic, rx_logic) = broadcast::bounded(1);
    let (tx_view, rx_view) = broadcast::bounded(1);
    let (tx_container, rx_container) = mpmc::bounded(1);
    let (tx_input, rx_input) = mpmc::bounded(1);
    let (tx_cancel, rx_cancel) = broadcast::bounded(1);

    Component::from(app_view(
        &app,
        tx_logic.clone(),
        tx_cancel,
        rx_view,
        tx_container,
        tx_input,
        card_components,
    ))
    .with_logic(app_logic(
        app,
        tx_logic,
        rx_logic,
        rx_cancel,
        tx_view,
        rx_container,
        rx_input,
    ))
}

#[wasm_bindgen]
pub fn bench() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    app_component().build().unwrap().run()
}
