use yew::prelude::*;

pub struct Greet {
    title: String,
    msg: String,
}

impl Component for Greet {
    type Message = ();
    type Properties = ();

    fn create(_props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self {
            title: "xtask-wasm".to_string(),
            msg: "Hello world".to_string(),
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <h1>{self.title.clone()}</h1>
                <p>{self.msg.clone()}</p>
            </div>
        }
    }
}
