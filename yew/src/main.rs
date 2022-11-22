mod app;
mod routes;

mod text_input;

use app::App;

fn main() {
    yew::start_app::<App>();
}
