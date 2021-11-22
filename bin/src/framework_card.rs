use std::sync::Arc;

use log::trace;
use mogwai::{lock::RwLock, prelude::*};
use web_sys::{Document, KeyboardEvent, KeyboardEventInit};

#[derive(Clone, Debug)]
pub enum FrameworkState {
    Ready,
    Running,
    Done,
    Erred(String),
}

#[derive(Clone, Debug)]
pub enum CreateTodoMethod {
    Change,
    InputAndKeypress,
    InputAndKeyup,
    InputAndKeydown,
    Submit,
}

impl CreateTodoMethod {
    pub fn dispatch_events(&self, document: &Document, input: web_sys::HtmlInputElement) {
        let event = |name: &str, from: &HtmlElement| {
            let event = document
                .create_event("Event")
                .expect("could not create input event");
            event.init_event_with_bubbles_and_cancelable(name, true, true);
            from.dispatch_event(&event)
                .expect("could not dispatch event");
        };

        let keyboard_enter_event = |name: &str, from: &HtmlElement| {
            let mut init = KeyboardEventInit::new();
            init.bubbles(true);
            init.cancelable(true);
            init.which(13);
            init.key_code(13);
            init.key("Enter");
            let event = KeyboardEvent::new_with_keyboard_event_init_dict(name, &init)
                .expect("could not create keyboard event");
            from.dispatch_event(&event)
                .expect("could not dispatch event");
        };
        match self {
            CreateTodoMethod::Change => {
                event("change", &input);
            }
            CreateTodoMethod::InputAndKeypress => {
                event("input", &input);
                keyboard_enter_event("keypress", &input);
            }
            CreateTodoMethod::InputAndKeyup => {
                event("input", &input);
                keyboard_enter_event("keyup", &input);
            }
            CreateTodoMethod::InputAndKeydown => {
                event("input", &input);
                keyboard_enter_event("keydown", &input);
            }
            CreateTodoMethod::Submit => {
                event("input", &input);
                if let Some(form) = input.form() {
                    event("submit", &form);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct FrameworkFacade {
    tx_logic: broadcast::Sender<In>,
}

impl FrameworkFacade {
    pub fn create(card: FrameworkCard) -> (Self, Component<Dom>) {
        let (tx_logic, rx_logic) = broadcast::bounded(1);
        let (tx_view, rx_view) = broadcast::bounded(1);
        let comp = Component::from(view(&card, tx_logic.clone(), rx_view))
            .with_logic(logic(card, rx_logic, tx_view));
        (FrameworkFacade { tx_logic }, comp)
    }

    pub async fn set_state(&self, state: FrameworkState) {
        self.tx_logic
            .broadcast(In::ChangeState(state))
            .await
            .unwrap();
    }

    pub async fn set_enabled(&self, is_enabled: bool) {
        self.tx_logic
            .broadcast(In::IsEnabled(is_enabled))
            .await
            .unwrap();
    }

    pub async fn get_card(&self) -> FrameworkCard {
        let (tx, mut rx) = broadcast::bounded(1);
        self.tx_logic.broadcast(In::QueryCard(tx)).await.unwrap();
        rx.next().await.unwrap()
    }
}

#[derive(Clone)]
pub struct FrameworkCard {
    pub name: String,
    pub url: String,
    pub attributes: Vec<(String, String)>,
    pub is_enabled: bool,
    pub state: FrameworkState,
    pub create_todo_method: CreateTodoMethod,
    pub wait_for_input_focus: bool,
}

impl FrameworkCard {
    pub fn framework_attribute(&self, key: &str) -> Option<String> {
        for (attr, value) in self.attributes.iter() {
            if attr == key {
                return Some(value.clone());
            }
        }
        None
    }
}

#[derive(Clone)]
pub enum In {
    ChangeState(FrameworkState),
    ToggleEnabled,
    IsEnabled(bool),
    QueryCard(broadcast::Sender<FrameworkCard>),
}

#[derive(Clone, Debug)]
pub enum Out {
    ChangeState(FrameworkState),
    IsEnabled(bool),
}

fn toggle_btn_class(enabled: bool) -> String {
    if enabled {
        "btn btn-primary"
    } else {
        "btn btn-outline-secondary"
    }
    .to_string()
}

impl Out {
    fn error_state_msg(&self) -> Option<Option<String>> {
        if let Out::ChangeState(FrameworkState::Erred(msg)) = self {
            Some(Some(msg.clone()))
        } else {
            None
        }
    }

    fn toggle_button_class(&self) -> Option<String> {
        match self {
            Out::IsEnabled(enabled) => Some(toggle_btn_class(*enabled)),
            _ => None,
        }
    }
}

async fn logic(
    mut card: FrameworkCard,
    mut rx_logic: broadcast::Receiver<In>,
    tx_view: broadcast::Sender<Out>,
) {
    while let Some(msg) = rx_logic.next().await {
        match msg {
            In::QueryCard(tx) => {
                tx.broadcast(card.clone()).await.unwrap();
            }
            In::ChangeState(st) => {
                trace!("{} state change to {:?}", card.name, st);
                tx_view
                    .broadcast(Out::ChangeState(st.clone()))
                    .await
                    .unwrap();
                card.state = st.clone();
            }
            In::ToggleEnabled => {
                let is_enabled = !card.is_enabled;
                card.is_enabled = is_enabled;
                trace!("{} card toggled {}", card.name, card.is_enabled);

                tx_view
                    .broadcast(Out::IsEnabled(card.is_enabled))
                    .await
                    .unwrap();
            }
            In::IsEnabled(enabled) => {
                card.is_enabled = enabled;
                tx_view
                    .broadcast(Out::IsEnabled(card.is_enabled))
                    .await
                    .unwrap();
            }
        }
    }
}

fn view(
    card: &FrameworkCard,
    tx: broadcast::Sender<In>,
    rx: broadcast::Receiver<Out>,
) -> ViewBuilder<Dom> {
    builder! {
        <tr>
            <td>
                <button
                    class = (
                        toggle_btn_class(card.is_enabled),
                        rx.clone().filter_map(|msg| async move { msg.toggle_button_class() })
                    )
                    on:click = tx.sink().contra_map(|_| In::ToggleEnabled)
                />
            </td>
            <td>
                <a  class={(
                        "text-secondary",
                        rx.clone().filter_map(|msg| async move {
                            match msg {
                                Out::ChangeState(st) => Some(
                                    match st {
                                        FrameworkState::Ready => "text-secondary",
                                        FrameworkState::Running => "text-primary",
                                        FrameworkState::Done => "text-success",
                                        FrameworkState::Erred(_) => "text-danger",
                                    }
                                    .into(),
                                ),
                                _ => None,
                            }
                        })
                    )}
                    href=&card.url>
                    {&card.name}
                </a>
            </td>
            <td>
                {card
                 .attributes
                 .iter()
                 .find(|item| item.0 == "version")
                 .map(|item| &item.1)
                 .unwrap()
                }
            </td>
            <td>
                {card
                 .attributes
                 .iter()
                 .find(|item| item.0 == "language")
                 .map(|item| &item.1)
                 .unwrap()
                }
            </td>
            <td>
                {card
                 .attributes
                 .iter()
                 .find(|item| item.0.contains("vdom"))
                 .map(|item| &item.1)
                 .unwrap()
                }
            </td>
            <td>"???"</td>
            <td>"???"</td>
            <td>
                <dd class="col-sm-12">
                    {(
                        "...",
                        rx.clone().filter_map(|msg| async move {
                            msg.error_state_msg()
                                .map(|may_err| may_err.unwrap_or("...".to_string()))
                        }),
                    )}
                </dd>
            </td>
        </tr>
    }
}

pub fn all_cards() -> Vec<FrameworkCard> {
    vec![
        FrameworkCard {
            name: "mogwai 0.1".into(),
            url: "frameworks/mogwai-0.1/index.html".into(),
            attributes: vec![
                ("language".into(), "rust".into()),
                ("version".into(), "0.1.5".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::Change,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "mogwai 0.2".into(),
            url: "frameworks/mogwai-0.2/index.html".into(),
            attributes: vec![
                ("language".into(), "rust".into()),
                ("version".into(), "0.2.0".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::Change,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "mogwai 0.5".into(),
            url: "frameworks/mogwai-0.5/index.html".into(),
            attributes: vec![
                ("language".into(), "rust".into()),
                ("version".into(), "0.2.0".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::Change,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "sauron".into(),
            url: "frameworks/sauron/index.html".into(),
            attributes: vec![
                ("language".into(), "rust".into()),
                ("version".into(), "0.20.3".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "yew".into(),
            url: "frameworks/yew-0.10/index.html".into(),
            attributes: vec![
                ("language".into(), "rust".into()),
                ("version".into(), "0.10.0".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Backbone".into(),
            url: "frameworks/backbone/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "1.1.2".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Asterius".into(),
            url: "frameworks/asterius/index.html".into(),
            attributes: vec![
                ("language".into(), "haskell".into()),
                ("version".into(), "0".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: false,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Ember".into(),
            url: "frameworks/emberjs/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "1.4".into()),
                ("has vdom".into(), "?".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeyup,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Angular".into(),
            url: "frameworks/angularjs-perf/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "1.5.3".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::Submit,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Mithril".into(),
            url: "frameworks/mithril/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "0.1.0".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Mithril2".into(),
            url: "frameworks/mithril-2/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "2.0.4".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeypress,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Elm".into(),
            url: "frameworks/elm17/index.html".into(),
            attributes: vec![
                ("language".into(), "elm".into()),
                ("version".into(), "0.17".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Preact".into(),
            url: "frameworks/preact/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "8.1.0".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "vanilla".into(),
            url: "frameworks/vanilla-es6/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "none".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: false,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Ractive".into(),
            url: "frameworks/ractive/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "0.3.9".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Knockout".into(),
            url: "frameworks/knockoutjs/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "3.1.0".into()),
                ("has vdom".into(), "no".into()),
            ],
            is_enabled: false,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Vue".into(),
            url: "frameworks/vue/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "1.0.24".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: false,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::Change,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Mercury".into(),
            url: "frameworks/mercury/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "3.1.7".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "React".into(),
            url: "frameworks/react/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "15.0.2".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "Om".into(),
            url: "frameworks/om/index.html".into(),
            attributes: vec![
                ("language".into(), "clojurescript".into()),
                ("version".into(), "0.5".into()),
                ("has vdom".into(), "yes".into()),
            ],
            is_enabled: true,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
        FrameworkCard {
            name: "choo".into(),
            url: "frameworks/choo/index.html".into(),
            attributes: vec![
                ("language".into(), "javascript".into()),
                ("version".into(), "1.3.0".into()),
                ("no vdom".into(), "still diffs".into()),
            ],
            is_enabled: false,
            state: FrameworkState::Ready,
            create_todo_method: CreateTodoMethod::InputAndKeydown,
            wait_for_input_focus: false,
        },
    ]
}
