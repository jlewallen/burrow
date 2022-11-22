mod app;
mod conn;
mod history;
mod home;
mod login;
mod routes;
mod services;
mod text_input;

use app::App;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}
