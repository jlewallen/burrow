mod app;
mod conn;
mod home;
mod login;
mod routes;
mod services;

mod command_line;
mod history;
mod open_web_socket;
mod text_input;

use app::App;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
