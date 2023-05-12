#[derive(Default, Debug, PartialEq, Eq)]
pub enum Command {
    #[default]
    NoOp,
    FetchPodcastFeed(String),
}

pub fn parse(s: &str) -> Command {
    let mut parts = s.split(" ");

    let op = parts.next();
    if op.is_none() {
        return Command::NoOp;
    }
    let op = op.unwrap();

    let args = parts.map(str::to_string).collect::<Vec<String>>();

    match op {
        "/load" => Command::FetchPodcastFeed(args.join("")),
        _ => Command::NoOp,
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::input::Command;

    use super::parse;

    #[test]
    fn parses_fetch_podcast_feed() {
        let input = "/load https://google.com something else";
        if let Command::FetchPodcastFeed(url) = parse(input) {
            assert_eq!(url, "https://google.comsomethingelse") // TODO: should this only take the first arg?
        } else {
            panic!("Did not get the right InputType");
        }
    }

    #[test]
    fn parses_fetch_podcast_feed_no_args() {
        let input = "/load";
        if let Command::FetchPodcastFeed(url) = parse(input) {
            assert_eq!(url, "")
        } else {
            panic!("Did not get the right InputType");
        }
    }

    #[test]
    fn parses_no_op() {
        let input = "something";
        assert_eq!(parse(input), Command::NoOp);
    }

    #[test]
    fn parses_empty_input() {
        assert_eq!(parse(""), Command::NoOp);
    }
}
