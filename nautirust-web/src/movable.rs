use yew::{html, Children, Component, NodeRef, Properties};

pub struct Moveable {
    node_ref: NodeRef,
}

#[derive(Clone, Properties, PartialEq)]
pub struct MoveableProps {
    pub children: Children,
}

impl Component for Moveable {
    type Message = ();
    type Properties = MoveableProps;

    fn create(_: &yew::Context<Self>) -> Self {
        Self {
            node_ref: NodeRef::default(),
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        html! {
            <div class="moveable">
            { ctx.props().children.clone() }
            </div>
        }
    }
}
