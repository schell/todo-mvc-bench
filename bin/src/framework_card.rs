use log::trace;
use mogwai::prelude::*;
use web_sys::{Event, Document, KeyboardEvent, KeyboardEventInit};


#[derive(Clone, Debug)]
pub enum FrameworkState {
  Ready,
  Erred(String),
}


#[derive(Clone, Debug)]
pub enum CreateTodoMethod {
  Change,
  Keydown,
  Keypress,
  InputAndKeypress,
  InputAndKeyup,
  InputAndKeydown,
  Submit
}


impl CreateTodoMethod {
  pub fn dispatch_events(&self, document: &Document, input: &HtmlInputElement) {
    let event = |name: &str, from: &HtmlElement| {
      let event =
        document
        .create_event("Event")
        .expect("could not create input event");
      event.init_event_with_bubbles_and_cancelable(name, true, true);
      from
        .dispatch_event(&event)
        .expect("could not dispatch event");
    };

    let keyboard_enter_event = |name:&str, from: &HtmlElement| {
      let mut init = KeyboardEventInit::new();
      init.bubbles(true);
      init.cancelable(true);
      init.which(13);
      init.key_code(13);
      init.key("Enter");
      let event =
        KeyboardEvent::new_with_keyboard_event_init_dict(
          name,
          &init
        )
        .expect("could not create keyboard event");
      let event =
        event
        .dyn_into::<Event>()
        .expect("could not cast keyboard event");
      from
        .dispatch_event(&event)
        .expect("could not dispatch event");
    };
    match self {
      CreateTodoMethod::Change => {
        event("change", input);
      }
      CreateTodoMethod::Keydown => {
        keyboard_enter_event("keydown", input);
      }
      CreateTodoMethod::Keypress => {
        keyboard_enter_event("keypress", input);
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
  pub create_todo_method: CreateTodoMethod
}


impl FrameworkCard {
  pub fn new(
    name: &str,
    url: &str,
    attributes: &[(&str, &str)],
    is_enabled: bool,
    create_todo_method: CreateTodoMethod,
  ) -> Self {
    let attributes =
      attributes
      .iter()
      .map(|(s,b)| (s.to_string(), b.to_string()))
      .collect::<Vec<_>>();

    FrameworkCard {
      name: name.into(),
      url: url.into(),
      attributes,
      is_enabled,
      state: FrameworkState::Ready,
      create_todo_method
    }
  }
}


#[derive(Clone)]
pub enum In {
  ChangeState(FrameworkState),
  ToggleEnabled
}


#[derive(Clone)]
pub enum Out {
  ChangeState(FrameworkState),
  IsEnabled(bool)
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
    _sub: &Subscriber<Self::ModelMsg>
  ) {
    match msg {
      In::ChangeState(st) => {
        trace!("{} state change to {:?}", self.name, st);
        tx.send(&Out::ChangeState(st.clone()));
        self.state = st.clone();
      }
      In::ToggleEnabled => {
        self.is_enabled = !self.is_enabled;
        tx.send(&Out::IsEnabled(self.is_enabled))
      }
    }
  }

  fn view(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>
  ) -> Gizmo<HtmlElement> {
    // TODO: Add status badge for framework card
    // https://getbootstrap.com/docs/4.3/components/badge/
    div()
      .class("card mb-4 shadow-sm")
      .with(
        div()
          .class("card-header")
          .with(
            h4()
              .class("my-0 font-weight-normal")
              .with(
                a()
                  .attribute("href", &self.url)
                  .text(&self.name)
              )
          )
      )
      .with(
        div()
          .class("card-body")
          .with(
            {
              let mut dl = dl().class("row list-unstyled mt-3 mb-4");
              for (attr, val) in self.attributes.iter() {
                let dt =
                  dt()
                  .class("col-sm-6")
                  .text(attr);
                let dd =
                  dd()
                  .class("col-sm-6")
                  .text(val);
                dl = dl.with(dt).with(dd);
              }
              dl
            }
            .with(
              dd()
                .class("col-sm-12")
                .rx_text(
                  "...",
                  rx.branch_filter_map(|msg| {
                    msg
                      .error_state_msg()
                      .map(|may_err| may_err.unwrap_or("...".to_string()))
                  })
                )
            )
          )
          .with(
            button()
              .attribute("type", "button")
              .rx_class(
                "btn btn-lg btn-block btn-primary",
                rx.branch_filter_map(|msg| {
                  if let Out::IsEnabled(is_enabled) = msg {
                    let btn_color =
                      if *is_enabled {
                        "btn-primary"
                      } else {
                        "btn-warning"
                      }.to_string();
                    Some(format!("btn btn-lg btn-block {}", btn_color))
                  } else {
                    None
                  }
                })
              )
              .rx_text("Enabled", rx.branch_filter_map(|msg| {
                if let Out::IsEnabled(is_enabled) = msg {
                  Some(
                    if *is_enabled {
                      "Enabled"
                    } else {
                      "Disabled"
                    }.to_string()
                  )
                } else {
                  None
                }
              }))
              .tx_on("click", tx.contra_map(|_| In::ToggleEnabled))
          )
      )
  }
}


pub fn all_cards() -> Vec<FrameworkCard> {
  vec![
    FrameworkCard::new(
      "sauron",
      "frameworks/sauron/index.html",
      &[
        ("language", "rust"),
        ("version", "0.20.3"),
        ("has vdom", "yes"),
      ],
      true,
      CreateTodoMethod::InputAndKeypress
    ),
    FrameworkCard::new(
      "mogwai",
      "frameworks/mogwai/index.html",
      &[
        ("language", "rust"),
        ("version", "0.1.5"),
        ("has vdom", "no"),
      ],
      true,
      CreateTodoMethod::Change
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
      CreateTodoMethod::InputAndKeypress
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
      CreateTodoMethod::InputAndKeypress
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
      CreateTodoMethod::InputAndKeyup
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
      CreateTodoMethod::Submit
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
      CreateTodoMethod::InputAndKeypress
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
      CreateTodoMethod::InputAndKeypress
    ),
    FrameworkCard::new(
      "Elm",
      "frameworks/elm17/index.html",
      &[
        ("language", "javascript"),
        ("version", "0.17"),
        ("has vdom", "yes"),
      ],
      true,
      CreateTodoMethod::InputAndKeydown
    ),
  ]
}
