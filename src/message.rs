use rss::Channel;
use url::Url;

pub enum Request {
    Feed(Url)
}

pub enum Response {
    Feed(Channel)
}