use log::trace;
use mogwai::prelude::*;
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
    pub fn dispatch_events(&self, document: &Document, input: &HtmlInputElement) {
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
                event("change", input);
            }
            CreateTodoMethod::InputAndKeypress => {
                event("input", input);
                keyboard_enter_event("keypress", input);
            }
            CreateTodoMethod::InputAndKeyup => {
                event("input", input);
                keyboard_enter_event("keyup", input);
            }
            CreateTodoMethod::InputAndKeydown => {
                event("input", input);
                keyboard_enter_event("keydown", input);
            }
            CreateTodoMethod::Submit => {
                event("input", input);
                if let Some(form) = input.form() {
                    event("submit", &form);
                }
            }
        }
    }
}

// TODO: Allow disabling
#[derive(Clone)]
pub struct FrameworkCard {
    pub name: String,
    pub url: String,
    pub attributes: Vec<(String, String)>,
    pub is_enabled: bool,
    pub state: FrameworkState,
    pub create_todo_method: CreateTodoMethod,
}

impl FrameworkCard {
    pub fn new(
        name: &str,
        url: &str,
        attributes: &[(&str, &str)],
        is_enabled: bool,
        create_todo_method: CreateTodoMethod,
    ) -> Self {
        let attributes = attributes
            .iter()
            .map(|(s, b)| (s.to_string(), b.to_string()))
            .collect::<Vec<_>>();

        FrameworkCard {
            name: name.into(),
            url: url.into(),
            attributes,
            is_enabled,
            state: FrameworkState::Ready,
            create_todo_method,
        }
    }

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
}

#[derive(Clone, Debug)]
pub enum Out {
    ChangeState(FrameworkState),
    IsEnabled(bool),
}

impl Out {
    fn error_state_msg(&self) -> Option<Option<String>> {
        if let Out::ChangeState(FrameworkState::Erred(msg)) = self {
            Some(Some(msg.clone()))
        } else {
            None
        }
    }
}

impl Component for FrameworkCard {
    type ModelMsg = In;
    type ViewMsg = Out;
    type DomNode = HtmlElement;

    fn update(
        &mut self,
        msg: &Self::ModelMsg,
        tx: &Transmitter<Self::ViewMsg>,
        _sub: &Subscriber<Self::ModelMsg>,
    ) {
        match msg {
            In::ChangeState(st) => {
                trace!("{} state change to {:?}", self.name, st);
                tx.send(&Out::ChangeState(st.clone()));
                self.state = st.clone();
            }
            In::ToggleEnabled => {
                self.is_enabled = !self.is_enabled;
                trace!("{} card toggled {}", self.name, self.is_enabled);
                tx.send(&Out::IsEnabled(self.is_enabled))
            }
            In::IsEnabled(enabled) => {
                self.is_enabled = *enabled;
                tx.send(&Out::IsEnabled(self.is_enabled))
            }
        }
    }

    fn view(
        &self,
        tx: Transmitter<Self::ModelMsg>,
        rx: Receiver<Self::ViewMsg>,
    ) -> Gizmo<HtmlElement> {
        tr().with(
            td().with(
                // Add the input inline in the DOM
                input()
                    .attribute("type", "checkbox")
                    .tx_on("click", tx.contra_map(|_| In::ToggleEnabled))
                    .rx_checked(
                        self.is_enabled,
                        rx.branch_filter_map(|msg| match msg {
                            Out::IsEnabled(is_enabled) => Some(*is_enabled),
                            _ => None,
                        }),
                    ),
            ),
        )
        .with(
            td().with(a().attribute("href", &self.url).text(&self.name).rx_class(
                "text-secondary",
                rx.branch_filter_map(|msg| {
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
                }),
            )),
        )
        .with(
            td().text(
                self.attributes
                    .iter()
                    .find(|item| item.0 == "version")
                    .map(|item| &item.1)
                    .unwrap_throw(),
            ),
        )
        .with(
            td().text(
                self.attributes
                    .iter()
                    .find(|item| item.0 == "language")
                    .map(|item| &item.1)
                    .unwrap_throw(),
            ),
        )
        .with(
            td().text(
                self.attributes
                    .iter()
                    .find(|item| item.0.contains("vdom"))
                    .map(|item| &item.1)
                    .unwrap_throw(),
            ),
        )
        .with(td().text("???"))
        .with(td().text("???"))
        .with(td().with(dd().class("col-sm-12").rx_text(
            "...",
            rx.branch_filter_map(|msg| {
                msg.error_state_msg()
                    .map(|may_err| may_err.unwrap_or("...".to_string()))
            }),
        )))
    }
}

pub fn all_cards() -> Vec<FrameworkCard> {
    vec![
        FrameworkCard::new(
            "mogwai 0.1",
            "frameworks/mogwai-0.1/index.html",
            &[
                ("language", "rust"),
                ("version", "0.1.5"),
                ("has vdom", "no"),
            ],
            true,
            CreateTodoMethod::Change,
        ),
        FrameworkCard::new(
            "mogwai 0.2",
            "frameworks/mogwai/index.html",
            &[
                ("language", "rust"),
                ("version", "0.2.0"),
                ("has vdom", "no"),
            ],
            true,
            CreateTodoMethod::Change,
        ),
        FrameworkCard::new(
            "sauron",
            "frameworks/sauron/index.html",
            &[
                ("language", "rust"),
                ("version", "0.20.3"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "yew",
            "frameworks/yew-0.10/index.html",
            &[
                ("language", "rust"),
                ("version", "0.10.0"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "Backbone",
            "frameworks/backbone/index.html",
            &[
                ("language", "javascript"),
                ("version", "1.1.2"),
                ("has vdom", "no"),
            ],
            true,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "Asterius",
            "frameworks/asterius/index.html",
            &[
                ("language", "haskell"),
                ("version", "0"),
                ("has vdom", "no"),
            ],
            false,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "Ember",
            "frameworks/emberjs/index.html",
            &[
                ("language", "javascript"),
                ("version", "1.4"),
                ("has vdom", "?"),
            ],
            true,
            CreateTodoMethod::InputAndKeyup,
        ),
        FrameworkCard::new(
            "Angular",
            "frameworks/angularjs-perf/index.html",
            &[
                ("language", "javascript"),
                ("version", "1.5.3"),
                ("has vdom", "no"),
            ],
            true,
            CreateTodoMethod::Submit,
        ),
        FrameworkCard::new(
            "Mithril",
            "frameworks/mithril/index.html",
            &[
                ("language", "javascript"),
                ("version", "0.1.0"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "Mithril2",
            "frameworks/mithril-2/index.html",
            &[
                ("language", "javascript"),
                ("version", "2.0.4"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeypress,
        ),
        FrameworkCard::new(
            "Elm",
            "frameworks/elm17/index.html",
            &[
                ("language", "elm"),
                ("version", "0.17"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "Preact",
            "frameworks/preact/index.html",
            &[
                ("language", "javascript"),
                ("version", "8.1.0"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "vanilla",
            "frameworks/vanilla-es6/index.html",
            &[
                ("language", "javascript"),
                ("version", "none"),
                ("has vdom", "no"),
            ],
            false,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "Ractive",
            "frameworks/ractive/index.html",
            &[
                ("language", "javascript"),
                ("version", "0.3.9"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "Knockout",
            "frameworks/knockoutjs/index.html",
            &[
                ("language", "javascript"),
                ("version", "3.1.0"),
                ("has vdom", "no"),
            ],
            false,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "Vue",
            "frameworks/vue/index.html",
            &[
                ("language", "javascript"),
                ("version", "1.0.24"),
                ("has vdom", "yes"),
            ],
            false,
            CreateTodoMethod::Change,
        ),
        FrameworkCard::new(
            "Mercury",
            "frameworks/mercury/index.html",
            &[
                ("language", "javascript"),
                ("version", "3.1.7"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "React",
            "frameworks/react/index.html",
            &[
                ("language", "javascript"),
                ("version", "15.0.2"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "Om",
            "frameworks/om/index.html",
            &[
                ("language", "clojurescript"),
                ("version", "0.5"),
                ("has vdom", "yes"),
            ],
            true,
            CreateTodoMethod::InputAndKeydown,
        ),
        FrameworkCard::new(
            "choo",
            "frameworks/choo/index.html",
            &[
                ("language", "javascript"),
                ("version", "1.3.0"),
                ("no vdom", "still diffs"),
            ],
            false,
            CreateTodoMethod::InputAndKeydown,
        ),
    ]
}
