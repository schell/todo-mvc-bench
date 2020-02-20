use log::trace;
use mogwai::prelude::*;
use web_sys::{Event, Document, KeyboardEvent, KeyboardEventInit};


#[derive(Clone, Debug)]
pub enum FrameworkState {
  Ready,
  Erred(String)
}


#[derive(Clone)]
pub enum CreateTodoMethod {
  Change,
  Keydown,
  Keypress
}


impl CreateTodoMethod {
  pub fn create_event(&self, document: &Document) -> Event {

    let event =
      match self {
        CreateTodoMethod::Change => {
          let event =
            document
            .create_event("Event")
            .expect("could not create change event");
          event.init_event_with_bubbles_and_cancelable("change", true, true);
          event
        }
        CreateTodoMethod::Keydown => {
          let mut init = KeyboardEventInit::new();
          init.bubbles(true);
          init.cancelable(true);
          init.which(13);
          let event =
            KeyboardEvent::new_with_keyboard_event_init_dict(
              "keydown",
              &init
            )
            .expect("could not create keyboard event");
          event
            .dyn_into::<Event>()
            .expect("could not cast keyboard event")
        }
        CreateTodoMethod::Keypress => {
          let mut init = KeyboardEventInit::new();
          init.bubbles(true);
          init.cancelable(true);
          init.which(13);
          let event =
            KeyboardEvent::new_with_keyboard_event_init_dict(
              "keypress",
              &init
            )
            .expect("could not create keyboard event");
          event
            .dyn_into::<Event>()
            .expect("could not cast keyboard event")
        }
      };

    event
  }
}


// TODO: Allow disabling
pub struct FrameworkCard {
  pub name: String,
  pub version: String,
  pub language: String,
  pub url: String,
  pub attributes: Vec<(String, bool)>,
  pub is_enabled: bool,
  pub state: FrameworkState,
  pub create_todo_method: CreateTodoMethod
}


impl FrameworkCard {
  pub fn new(
    name: &str,
    version: &str,
    language: &str,
    url: &str,
    attributes: &[(&str, bool)],
    is_enabled: bool,
    create_todo_method: CreateTodoMethod,
  ) -> Self {
    let attributes =
      attributes
      .iter()
      .map(|(s,b)| (s.to_string(), *b))
      .collect::<Vec<_>>();

    FrameworkCard {
      name: name.into(),
      version: version.into(),
      language: language.into(),
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
  ChangeState(FrameworkState)
}


#[derive(Clone)]
pub enum Out {
  ChangeState(FrameworkState)
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
    }
  }

  fn builder(
    &self,
    _tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>
  ) -> GizmoBuilder {
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
              .text(&self.name)
          )
      )
      .with(
        div()
          .class("card-body")
          .with(
            h1()
              .class("card-title pricing-card-title")
              .with(
                small()
                  .class("text-muted")
                  .text(&self.language)
              )
          )
          .with(
            dl()
              .class("row list-unstyled mt-3 mb-4")
              .with_many(
                self
                  .attributes
                  .iter()
                  .flat_map(|(attr, val)| {
                    let val_str =
                      if *val {
                        "yes"
                      } else {
                        "no"
                      };
                    vec![
                      dt()
                        .class("col-sm-6")
                        .text(attr),
                      dd()
                        .class("col-sm-6")
                        .text(val_str)
                    ]
                  })
                  .collect::<Vec<_>>()
              )
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
              .class("btn btn-lg btn-block btn-primary")
              .rx_text("Disable", rx.branch_filter_map(|_msg| None))
          )
      )
  }
}


pub fn all_cards() -> Vec<FrameworkCard> {
  vec![
    FrameworkCard::new(
      "mogwai",
      "0.1.5",
      "rust",
      "frameworks/mogwai/index.html",
      &[
        ("has vdom", false),
        ("is elm like", true)
      ],
      true,
      CreateTodoMethod::Change
    ),
    FrameworkCard::new(
      "sauron",
      "0.20.1",
      "rust",
      "frameworks/sauron/index.html",
      &[
        ("has vdom", true),
        ("is elm like", true)
      ],
      true,
      CreateTodoMethod::Keypress
    ),
    FrameworkCard::new(
      "yew",
      "0.10.0",
      "rust",
      "frameworks/yew-0.10/index.html",
      &[
        ("has vdom", true),
        ("is elm like", true)
      ],
      true,
      CreateTodoMethod::Keypress
    ),
    FrameworkCard::new(
      "Backbone",
      "1.1.2",
      "javascript",
      "frameworks/backbone/index.html",
      &[
        ("has vdom", false),
        ("is elm like", false)
      ],
      true,
      CreateTodoMethod::Keypress
    ),

  ]
}
