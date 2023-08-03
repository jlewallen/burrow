use gloo_timers::callback::Timeout;
use web_sys::HtmlElement;
use yew::prelude::*;

use crate::shared::history_items::HistoryItems;
use crate::shared::CommandLine;
use crate::shared::Evaluator;
use crate::shared::LogoutButton;
use crate::shared::SessionHistory;

pub enum Msg {
    UpdateHistory(SessionHistory),
    UpdateEvaluator(Evaluator),
    Refresh,
}

pub struct Home {
    refs: Vec<NodeRef>,
    history: Option<SessionHistory>,
    evaluator: Evaluator,
    _history_listener: ContextHandle<SessionHistory>,
    _evaluator_listener: ContextHandle<Evaluator>,
}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (history, history_listener) = ctx
            .link()
            .context::<SessionHistory>(ctx.link().callback(|history| Msg::UpdateHistory(history)))
            .expect("No history context");

        let (evaluator, evaluator_listener) = ctx
            .link()
            .context::<Evaluator>(
                ctx.link()
                    .callback(|evaluator| Msg::UpdateEvaluator(evaluator)),
            )
            .expect("No evalutor context");

        Self {
            history: Some(history),
            refs: vec![NodeRef::default()],
            evaluator,
            _history_listener: history_listener,
            _evaluator_listener: evaluator_listener,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::UpdateEvaluator(evaluator) => {
                log::info!("update-evaluator");

                self.evaluator = evaluator;

                true
            }
            Self::Message::UpdateHistory(history) => {
                self.history = Some(history);

                log::info!("update-history");

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

                log::debug!(
                    "update-history:refresh ({}, {})",
                    upper_div.scroll_top(),
                    upper_div.scroll_height()
                );

                upper_div.set_scroll_top(upper_div.scroll_height());

                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let evaluator = self.evaluator.clone();

        if let Some(history) = self.history.clone() {
            html! {
                <div id="hack">
                    <div id="upper" ref={&self.refs[0]}>
                        <div id="main"><HistoryItems history={history.clone()} /></div>
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
                            <div class="bottom-bar">
                                <CommandLine oncommand={move |line: String| evaluator.evaluate(line.clone())} />
                                <LogoutButton />
                            </div>
                        </div>
                    </div>
                </div>
            }
        } else {
            html! { <div> {{ "Busy" }} </div> }
        }
    }
}
