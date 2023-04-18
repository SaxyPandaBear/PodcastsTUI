mod feed;
mod message;
mod trace;

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
use tracing::{event as trace_event, instrument, span, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use unicode_width::UnicodeWidthStr;

// App holds the state of the application
// TODO: persist application state about podcast that is loaded.
#[derive(Default, Debug)]
struct App {
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
        trace_event!(Level::DEBUG, idx = i);
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
        trace_event!(Level::DEBUG, idx = i);
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
        thread::sleep(Duration::new(0, 1000));
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
        terminal.draw(|f| ui(f, &mut app, ui_rx))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                // quit
                KeyCode::Esc => return Ok(()),
                // submit data
                KeyCode::Enter => {
                    trace_event!(
                        Level::INFO,
                        "Submitting request for display mode {display:?}",
                        display = app.display_action
                    );
                    match app.display_action {
                        DisplayAction::Input => {
                            // submit a message to data layer
                            let msg = app.input.drain(..).collect::<String>();
                            if let Ok(u) = url::Url::parse(msg.as_str()) {
                                // TODO: handle error
                                trace_event!(Level::INFO, "Fetch RSS feed from {url}", url = msg);
                                data_tx.send(message::Request::Feed(u));
                                app.display_action = DisplayAction::ListEpisodes;
                            }
                        }
                        DisplayAction::ListEpisodes => {
                            let item: Option<Item> =
                                app.channel.as_ref().map(|c| c.items()).and_then(|items| {
                                    app.state.selected().and_then(|idx| items.get(idx)).cloned()
                                    // TODO: there must be a more idiomatic way
                                });
                            trace_event!(
                                Level::INFO,
                                "Load podcast episode {exists}",
                                exists = item.is_some()
                            );
                            if item.is_some() {
                                app.display_action = DisplayAction::DescribeEpisode;
                            }
                            data_tx.send(message::Request::Episode(item));
                        }
                        DisplayAction::DescribeEpisode => {
                            // TODO: idk what should happen here yet. probably need to have another list of options.
                            trace_event!(Level::INFO, "Load episode");
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

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App, rx: &Receiver<message::Response>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(4)
        .constraints(
            [
                Constraint::Percentage(8),  // controls
                Constraint::Percentage(10), // input box
                Constraint::Percentage(8),  // output title
                Constraint::Percentage(84), // output contents
            ]
            .as_ref(),
        )
        .split(f.size());

    let (msg, style) = (
        vec![
            Span::styled("Podcasts::", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Press "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to exit, "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to input"),
        ],
        Style::default(),
    );
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let input = Paragraph::new(app.input.as_ref())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);

    // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
    f.set_cursor(
        // Put cursor past the end of the input text
        chunks[1].x + app.input.width() as u16 + 1,
        // Move one line down, from the border to the input line
        chunks[1].y + 1,
    );

    // Output area
    if let Ok(r) = rx.try_recv() {
        match r {
            message::Response::Feed(c) => {
                app.channel = Some(c);
                // loaded a channel, so next action should be to select an episode
            }
            message::Response::Episode(e) => {
                app.item = Some(e);
            }
        }
    }

    match app.display_action {
        DisplayAction::ListEpisodes | DisplayAction::Input => {
            display_feed_episodes(f, app, chunks[2], chunks[3]);
        }
        DisplayAction::DescribeEpisode => {
            display_episode_details(f, app, chunks[2], chunks[3]);
        }
    };
}

#[tokio::main]
#[instrument]
async fn handle_user_input(
    responder: &Sender<message::Response>,
    receiver: &Receiver<message::Request>,
) {
    if let Ok(r) = receiver.try_recv() {
        trace_event!(Level::INFO, "{:?}", r);
        match r {
            message::Request::Feed(u) => {
                trace_event!(Level::INFO, "received feed request");
                if let Ok(c) = feed::get_feed(u).await {
                    // TODO: error handling
                    responder.send(message::Response::Feed(c));
                }
            }
            message::Request::Episode(e) => {
                trace_event!(Level::INFO, "received episode request");
                if let Some(i) = e {
                    // don't need to load anything, just pass it back to the UI
                    responder.send(message::Response::Episode(i));
                }
            }
        }
    }
}

fn display_feed_episodes<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    title_area: Rect,
    content_area: Rect,
) {
    let span = span!(Level::TRACE, "render_feed");
    trace_event!(parent: &span, Level::TRACE, "rendering podcast episodes");
    let contents = app
        .channel
        .as_ref()
        .and_then(|c| Some(c.items()))
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let content = vec![Spans::from(Span::raw(format!(
                "{}: {}",
                idx,
                item.title.as_deref().unwrap_or("Title missing!")
            )))];
            ListItem::new(content)
        })
        .collect::<Vec<ListItem>>();

    let podcast_name = app.channel.as_ref().map(|c| c.title()).unwrap_or("[Title]");

    trace_event!(
        parent: &span,
        Level::DEBUG,
        num_episodes = contents.len(),
        name = podcast_name
    );

    let podcast_name = Paragraph::new(podcast_name);
    let contents = List::new(contents)
        .block(Block::default().borders(Borders::ALL).title("Episodes"))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::SLOW_BLINK),
        )
        .highlight_symbol(">>");

    f.render_widget(podcast_name, title_area);
    f.render_stateful_widget(contents, content_area, &mut app.state);
}

fn display_episode_details<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    title_area: Rect,
    content_area: Rect,
) {
    let span = span!(Level::TRACE, "render_episode");
    trace_event!(parent: &span, Level::TRACE, "rendering episode details");

    let episode_name = app
        .item
        .as_ref()
        .and_then(|i| i.title())
        .unwrap_or("[Episode Title]")
        .to_string();
    let description = app
        .item
        .as_ref()
        .and_then(|i| i.description())
        .unwrap_or("Description")
        .to_string();
    let description = html2text::from_read(description.as_bytes(), content_area.width.into());
    let audio_link = app
        .item
        .as_ref()
        .and_then(|i| i.enclosure.as_ref())
        .map(|e| e.url())
        .unwrap_or("[Audio URL]")
        .to_string();

    let text = vec![
        Spans::from(Span::styled(
            audio_link,
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from(Span::raw(description)),
    ];

    let episode_name = Paragraph::new(episode_name);
    let contents = Paragraph::new(text).wrap(Wrap { trim: true });
    f.render_widget(episode_name, title_area);
    f.render_widget(contents, content_area);
}
