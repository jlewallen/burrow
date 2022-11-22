mod app;
mod home;
mod login;
mod routes;
mod text_input;

use app::App;

fn main() {
    yew::start_app::<App>();
}
