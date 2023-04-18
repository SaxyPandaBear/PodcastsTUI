use rss::{Channel, Item};
use url::Url;

#[derive(Default, Debug)]
pub enum DisplayAction {
    #[default]
    Input,
    ListEpisodes,
    DescribeEpisode,
}

#[derive(Debug)]
pub enum Request {
    Feed(Url),
    Episode(Option<Item>)
}

#[derive(Debug)]
pub enum Response {
    Feed(Channel),
    Episode(Item)
}
