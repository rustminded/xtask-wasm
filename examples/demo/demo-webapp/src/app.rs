use yew::prelude::*;

pub struct Greet {
    msg: String,
}

impl Component for Greet {
    type Message = ();
    type Properties = ();

    fn create(_props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self {
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
                <p>{self.msg.clone()}</p>
            </div>
        }
    }
}
