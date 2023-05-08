mod feed;
mod message;
mod trace;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use message::DisplayAction;
use rss::{Channel, Item};
use std::sync::mpsc;
use std::thread;
use std::{
    error::Error,
    io,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};
use tracing::{debug, error, info, instrument, span, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    widgets::ListState,
    Frame, Terminal,
};
use ui::draw_main_layout;

use crate::ui::input::{self, InputType};

// App holds the state of the application
// TODO: persist application state about podcast that is loaded.
#[derive(Default, Debug)]
pub struct App {
    // Current value of the input box
    input: String,
    // Loaded podcast channel/feed
    channel: Option<Channel>,
    // Loaded podcast episode
    item: Option<Item>,
    state: ListState,
    // keep track of what to render on the UI across ticks
    display_action: DisplayAction,
}

impl App {
    // Select the next item. This will not be reflected until the widget is drawn in the
    // `Terminal::draw` callback using `Frame::render_stateful_widget`.
    #[instrument]
    pub fn next(&mut self) {
        let i = self
            .channel
            .as_ref()
            .map(|c| c.items())
            .and_then(|items| {
                self.state
                    .selected()
                    .map(|i| if i >= items.len() - 1 { 0 } else { i + 1 })
            })
            .unwrap_or_default();
        debug!(idx = i);
        self.state.select(Some(i));
    }

    // Select the previous item. This will not be reflected until the widget is drawn in the
    // `Terminal::draw` callback using `Frame::render_stateful_widget`.
    #[instrument]
    pub fn previous(&mut self) {
        let i: usize = self
            .channel
            .as_ref()
            .map(|c| c.items())
            .and_then(|items| {
                self.state
                    .selected()
                    .map(|i| if i == 0 { items.len() - 1 } else { i - 1 })
            })
            .unwrap_or_default();
        debug!(idx = i);
        self.state.select(Some(i));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // set up logging
    let file_appender = RollingFileAppender::new(Rotation::HOURLY, "/tmp", "podcasts.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_thread_names(true)
        .with_level(true);
    let span_filter = trace::TraceFilter::default();
    tracing_subscriber::registry()
        .with(fmt_layer.with_filter(span_filter))
        .init();

    // create app
    let app = App::default();

    // channel for publishing messages from the UI to the data thread
    let (data_tx, data_rx) = mpsc::channel::<message::Request>();
    // channel for publishing messages from the data thread to the UI
    let (ui_tx, ui_rx) = mpsc::channel::<message::Response>();

    // spawn data thread
    thread::spawn(move || loop {
        handle_user_input(&ui_tx, &data_rx);
        thread::sleep(Duration::new(0, 10000));
    });

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // run the main UI thread
    let res = run_app(&mut terminal, app, &data_tx, &ui_rx);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    data_tx: &Sender<message::Request>,
    ui_rx: &Receiver<message::Response>,
) -> io::Result<()> {
    loop {
        let span = span!(Level::TRACE, "draw");
        let _enter = span.enter();
        terminal.draw(|f| display(f, &mut app, ui_rx))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => {
                        info!("Closing application");
                        return Ok(());
                    }
                    // submit data
                    KeyCode::Enter => {
                        info!(
                            "Submitting request for display mode {display:?}",
                            display = app.display_action
                        );
                        // TODO: figure out how to handle different types of text input
                        match app.display_action {
                            DisplayAction::Input => {
                                // submit a message to data layer
                                let msg = app.input.drain(..).collect::<String>();
                                match input::parse(msg.as_ref()) {
                                    InputType::FetchPodcastFeed(url) => {
                                        info!("fetch podcast feed: {}", url);
                                        if let Ok(u) = url::Url::parse(url.as_str()) {
                                            info!("Fetch RSS feed from {url}", url = msg);
                                            let res = data_tx.send(message::Request::Feed(u));
                                            if res.is_err() {
                                                error!("failed to send message {:?}", res.unwrap_err());
                                            }
                                            app.display_action = DisplayAction::ListEpisodes;
                                        }
                                    }
                                    _ => {
                                        debug!("no op")
                                    }
                                }
                            }
                            DisplayAction::ListEpisodes => {
                                let item: Option<Item> =
                                    app.channel.as_ref().map(|c| c.items()).and_then(|items| {
                                        app.state.selected().and_then(|idx| items.get(idx)).cloned()
                                        // TODO: there must be a more idiomatic way
                                    });
                                info!("Load podcast episode {exists}", exists = item.is_some());
                                if item.is_some() {
                                    app.display_action = DisplayAction::DescribeEpisode;
                                }
                                let res = data_tx.send(message::Request::Episode(item));
                                if res.is_err() {
                                    error!("failed to send message {:?}", res.unwrap_err());
                                }
                            }
                            DisplayAction::DescribeEpisode => {
                                // TODO: idk what should happen here yet. probably need to have another list of options.
                                info!("Load episode");
                            }
                        }
                    }
                    // user input
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    // list selection
                    KeyCode::Up => {
                        app.previous();
                    }
                    KeyCode::Down => {
                        app.next();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn display<B: Backend>(f: &mut Frame<B>, app: &mut App, rx: &Receiver<message::Response>) {
    if let Ok(r) = rx.try_recv() {
        update_app_state(app, r)
    }

    draw_main_layout(f, app);
}

fn update_app_state(app: &mut App, msg: message::Response) {
    match msg {
        message::Response::Feed(c) => {
            app.channel = Some(c);
        }
        message::Response::Episode(e) => {
            app.item = Some(e);
        }
    }
}

#[tokio::main]
#[instrument]
async fn handle_user_input(
    responder: &Sender<message::Response>,
    receiver: &Receiver<message::Request>,
) {
    if let Ok(r) = receiver.try_recv() {
        info!("Request type: {:?}", r);
        match r {
            message::Request::Feed(u) => {
                info!("received feed request");
                if let Ok(c) = feed::get_feed(u).await {
                    // TODO: error handling
                    let res = responder.send(message::Response::Feed(c));
                    if res.is_err() {
                        error!("failed to send message: {:?}", res.unwrap_err());
                    }
                }
            }
            message::Request::Episode(e) => {
                info!("received episode request");
                if let Some(i) = e {
                    // don't need to load anything, just pass it back to the UI
                    let res = responder.send(message::Response::Episode(i));
                    if res.is_err() {
                        error!("failed to send message {:?}", res.unwrap_err());
                    }
                }
            }
        }
    }
}
