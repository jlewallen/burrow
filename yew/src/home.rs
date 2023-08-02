use gloo_console as console;
use gloo_timers::callback::Timeout;
use std::rc::Rc;
use web_sys::HtmlElement;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::command_line::CommandLine;
use crate::history::{History, SessionHistory};
use crate::open_web_socket::Evaluator;

pub enum Msg {
    UpdateHistory(Rc<SessionHistory>),
    Ignored,
    Refresh,
}

pub struct Home {
    refs: Vec<NodeRef>,
    evaluate_callback: Callback<String>,
    _dispatch: Dispatch<SessionHistory>,
}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (evaluator, _) = ctx
            .link()
            .context::<Evaluator>(ctx.link().callback(|_| Msg::Ignored))
            .expect("No evalutor context");

        let callback = ctx.link().callback(Msg::UpdateHistory);
        let dispatch = Dispatch::<SessionHistory>::subscribe(move |h| callback.emit(h));

        Self {
            refs: vec![NodeRef::default()],
            evaluate_callback: evaluator.callback.clone(),
            _dispatch: dispatch,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::UpdateHistory(_) => {
                let refresher = ctx.link().callback(|_| Self::Message::Refresh);

                let timeout = Timeout::new(25, move || {
                    refresher.emit(());
                });

                timeout.forget();

                true
            }
            Self::Message::Refresh => {
                let upper = &self.refs[0];
                let upper_div = &upper.cast::<HtmlElement>().unwrap();

                console::debug!(
                    "update-history:refresh (T, H)",
                    upper_div.scroll_top(),
                    upper_div.scroll_height()
                );

                upper_div.set_scroll_top(upper_div.scroll_height());

                true
            }
            _ => false,
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div id="hack">
                <div id="upper" ref={&self.refs[0]}>
                    <div id="main"><History /></div>
                </div>
                <div id="lower">
                    <div class="interactables">
                        <div class="editor" style="display: none;">
                            <div class="">
                                { "Tabs" }
                            </div>
                            <div class="buttons">
                                { "Buttons" }
                            </div>
                        </div>
                        <CommandLine oncommand={self.evaluate_callback.clone()} />
                    </div>
                </div>
            </div>
        }
    }
}
