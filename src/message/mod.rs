use rss::{Channel, Item};
use url::Url;

#[derive(Default, Debug, PartialEq)]
pub enum DisplayAction {
    #[default]
    Input, // TODO: this needs to change
    ListEpisodes,
    DescribeEpisode,
}

#[derive(Debug, PartialEq)]
pub enum Request {
    Feed(Url),
    Episode(Option<Item>),
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Feed(Channel),
    Episode(Item),
}
