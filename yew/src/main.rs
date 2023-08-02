mod app;
mod hooks;
mod pages;
mod routes;
mod services;
mod shared;

mod errors;
mod types;

use app::App;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
