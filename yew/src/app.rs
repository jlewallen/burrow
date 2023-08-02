use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::*;

use crate::open_web_socket::AlwaysOpenWebSocket;
use crate::user_context_provider::UserContextProvider;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <UserContextProvider>
                <AlwaysOpenWebSocket>
                    <div id="app">
                        <Switch<Route> render={switch}/>
                    </div>
                </AlwaysOpenWebSocket>
            </UserContextProvider>
        </BrowserRouter>
    }
}
