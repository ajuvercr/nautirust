use gloo_net::http::Request;
use serde::Deserialize;
use yew::prelude::*;

pub mod movable;

#[derive(Clone, PartialEq, Deserialize)]
struct Video {
    id:      usize,
    title:   String,
    speaker: String,
    url:     String,
}

#[derive(Clone, Properties, PartialEq)]
struct VideosDetailsProps {
    video: Video,
}

struct VideosComponent {
    videos:   Vec<Video>,
    selected: Option<usize>,
}

enum VideosMsg {
    Found(Vec<Video>),
    Select(usize),
}

impl From<Vec<Video>> for VideosMsg {
    fn from(x: Vec<Video>) -> Self {
        Self::Found(x)
    }
}

impl Component for VideosComponent {
    type Message = VideosMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            videos:   Vec::new(),
            selected: None,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        use movable::Moveable;
        let details = self.selected.as_ref().map(|video_idx| {
            html! {
                <Moveable>
                  <VideoDetails video={self.videos[*video_idx].clone()} />
                </Moveable>
            }
        });

        let list: Vec<_> = self.videos.iter().enumerate().map(|(i, video)| {
            let on_video_select = ctx.link().callback(move |_| VideosMsg::Select(i));

            html! {
                <p onclick={on_video_select}>{format!("{}: {}", video.speaker, video.title)}</p>
            }
        }).collect();

        html! {<> {for list} {for details} </>}
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let future = async move {
                Request::get("/tutorial/data.json")
                    .send()
                    .await
                    .unwrap()
                    .json::<Vec<Video>>()
                    .await
                    .unwrap()
            };

            ctx.link().send_future(future);
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            VideosMsg::Found(videos) => self.videos = videos,
            VideosMsg::Select(i) => self.selected = Some(i),
        }

        true
    }
}

#[function_component(VideoDetails)]
fn video_details(VideosDetailsProps { video }: &VideosDetailsProps) -> Html {
    html! {
        <div>
            <yew_feather::archive::Archive/>
            <h3>{ video.title.clone() }</h3>
            <img src="https://via.placeholder.com/640x360.png?text=Video+Player+Placeholder" alt="video thumbnail" />
        </div>
    }
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
        <h1>{ "Hello World" }</h1>
        <VideosComponent/>
        </>
    }
}

fn main() {
    yew::start_app::<App>();
}
