#[derive(Default, Debug, PartialEq, Eq)]
pub enum UserInput {
    #[default]
    NoOp,
    FetchPodcastFeed(String),
}

pub fn parse(s: &str) -> UserInput {
    let parts = s.split(" ");
    if parts.clone().count() < 1 {
        return UserInput::NoOp
    }

    let mut collection: Vec<&str> = parts.collect::<Vec<&str>>();
    match collection[0] {
        "/load" => {
            UserInput::FetchPodcastFeed(collection.drain(1..).collect())
        },
        _ => UserInput::NoOp
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::input::UserInput;

    use super::parse;

    #[test]
    fn parses_fetch_podcast_feed() {
        let input = "/load https://google.com something else";
        if let UserInput::FetchPodcastFeed(url) = parse(input) {
            assert_eq!(url, "https://google.comsomethingelse") // TODO: should this only take the first arg?
        } else {
            panic!("Did not get the right InputType");
        }
    }

    #[test]
    fn parses_no_op() {
        let input = "something";
        assert_eq!(parse(input), UserInput::NoOp)
    }
}
