use mogwai::prelude::*;

// TODO: Allow disabling
pub struct FrameworkCard {
  pub name: String,
  pub version: String,
  pub language: String,
  pub url: String,
  pub attributes: Vec<(String, bool)>,
  pub is_enabled: bool
}


impl FrameworkCard {
  pub fn new(
    name: &str,
    version: &str,
    language: &str,
    url: &str,
    attributes: &[(&str, bool)],
    is_enabled: bool
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
      is_enabled
    }
  }
}


#[derive(Clone)]
pub enum In {

}

#[derive(Clone)]
pub enum Out {

}


impl Component for FrameworkCard {
  type ModelMsg = In;
  type ViewMsg = Out;

  fn update(
    &mut self,
    _msg: &Self::ModelMsg,
    _tx: &Transmitter<Self::ViewMsg>,
    _sub: &Subscriber<Self::ModelMsg>
  ) {

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
              .class("list-unstyled mt-3 mb-4")
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
                        .text(attr),
                      dd()
                        .text(val_str)
                    ]
                  })
                  .collect::<Vec<_>>()
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
      true
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
      true
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
      true
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
      true
    ),

  ]
}
