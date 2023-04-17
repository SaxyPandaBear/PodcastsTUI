use rss::{Channel, Item};
use url::Url;

#[derive(Default)]
pub enum DisplayMode {
    #[default]
    EpisodeList,
    ItemContent,
}

pub enum Request {
    Feed(Url),
    Episode(Option<Item>)
}

pub enum Response {
    Feed(Channel),
    Episode(Item)
}

impl Response {
    pub fn display_type(&self) -> DisplayMode {
        match self {
            Response::Feed(_) => DisplayMode::EpisodeList,
            Response::Episode(_) => DisplayMode::ItemContent,
        }
    }
}