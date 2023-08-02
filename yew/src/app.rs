use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::*;

use crate::shared::{AlwaysOpenWebSocket, UserContextProvider};

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
