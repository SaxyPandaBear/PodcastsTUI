use rss::Channel;
use std::error::Error;
use url::Url;

pub async fn get_feed(u: Url) -> Result<Channel, Box<dyn Error>> {
    let content = reqwest::get(u.as_str())
        .await?
        .bytes()
        .await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}