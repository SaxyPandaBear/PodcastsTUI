use std::sync::mpsc::{Receiver, Sender};

use tracing::{debug, error, info, instrument};

use crate::{
    feed::get_feed,
    message::{DisplayAction, Request, Response},
    ui::input::UserInput,
    App,
};

#[tokio::main]
#[instrument]
pub async fn handle_background_request(responder: &Sender<Response>, receiver: &Receiver<Request>) {
    if let Ok(r) = receiver.try_recv() {
        info!("Request type: {:?}", r);
        match r {
            Request::Feed(u) => {
                info!("received feed request");
                if let Ok(c) = get_feed(u).await {
                    // TODO: error handling
                    let res = responder.send(Response::Feed(c));
                    if res.is_err() {
                        error!("failed to send message: {:?}", res.unwrap_err());
                    }
                }
            }
            Request::Episode(e) => {
                info!("received episode request");
                if let Some(i) = e {
                    // don't need to load anything, just pass it back to the UI
                    let res = responder.send(Response::Episode(i));
                    if res.is_err() {
                        error!("failed to send message {:?}", res.unwrap_err());
                    }
                }
            }
        }
    }
}

#[instrument]
pub fn handle_user_input(app: &mut App, sender: &Sender<Request>, i: UserInput) {
    match i {
        UserInput::FetchPodcastFeed(url) => {
            info!("fetch podcast feed: {}", url);
            if let Ok(u) = url::Url::parse(url.as_str()) {
                info!("Fetch RSS feed from {url}", url = u);
                let res = sender.send(Request::Feed(u));
                if res.is_err() {
                    error!("failed to send message {:?}", res.unwrap_err());
                }
                app.display_action = DisplayAction::ListEpisodes;
            }
        }
        _ => {
            debug!("no op {input:?}", input = i);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use url::{ParseError, Url};

    use crate::{
        message::{self, DisplayAction, Request},
        App, ui::input::UserInput,
    };

    use super::handle_user_input;

    #[test]
    fn send_load_request_publishes_feed_message() -> Result<(), ParseError> {
        let input = UserInput::FetchPodcastFeed("https://google.com".to_string());
        let mut app = App::default();
        let (data_tx, data_rx) = mpsc::channel::<message::Request>();

        let expected = Url::parse("https://google.com");
        assert_eq!(expected.clone()?.host_str(), Some("google.com"));
        
        assert_eq!(DisplayAction::Input, app.display_action);

        handle_user_input(&mut app, &data_tx, input);

        if let Ok(res) = data_rx.recv_timeout(Duration::from_secs(3)) {
            assert_eq!(res, Request::Feed(expected?));
        } else {
            panic!("did not receive a message in time");
        }

        assert_eq!(DisplayAction::ListEpisodes, app.display_action);
        Ok(())
    }

    #[test]
    fn send_no_op_does_nothing() {
        let input = UserInput::NoOp;
        let mut app = App::default();
        let (data_tx, data_rx) = mpsc::channel::<message::Request>();

        assert_eq!(DisplayAction::Input, app.display_action);

        handle_user_input(&mut app, &data_tx, input);
        if let Ok(_) = data_rx.recv_timeout(Duration::from_secs(3)) {
            panic!("should not have received a message")
        }

        // ensure state did not change
        assert_eq!(DisplayAction::Input, app.display_action);
    }
}
