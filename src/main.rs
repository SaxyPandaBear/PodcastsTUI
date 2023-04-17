mod feed;
mod message;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use message::DisplayMode;
use rss::{Channel, Item};
use std::sync::mpsc;
use std::thread;
use std::{
    error::Error,
    io,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget},
    Frame, Terminal,
};
use unicode_width::UnicodeWidthStr;

// App holds the state of the application
// TODO: persist application state about podcast that is loaded.
#[derive(Default)]
struct App {
    // Current value of the input box
    input: String,
    // Loaded podcast channel/feed
    channel: Option<Channel>,
    // Loaded podcast episode
    item: Option<Item>,
    state: ListState,
    // keep track of what to render on the UI across ticks
    display_mode: DisplayMode,
}

impl App {
    // Select the next item. This will not be reflected until the widget is drawn in the
    // `Terminal::draw` callback using `Frame::render_stateful_widget`.
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
        self.state.select(Some(i));
    }

    // Select the previous item. This will not be reflected until the widget is drawn in the
    // `Terminal::draw` callback using `Frame::render_stateful_widget`.
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
        self.state.select(Some(i));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
                    match app.display_mode {
                        DisplayMode::EpisodeList => {
                            // submit a message to data layer
                            let msg = app.input.drain(..).collect::<String>();
                            if let Ok(u) = url::Url::parse(msg.as_str()) {
                                // TODO: handle error
                                data_tx.send(message::Request::Feed(u));
                            }
                        }
                        DisplayMode::ItemContent => {
                            let item: Option<Item> =
                                app.channel.as_ref().map(|c| c.items()).and_then(|items| {
                                    app.state.selected().and_then(|idx| items.get(idx)).cloned()
                                    // TODO: there must be a more idiomatic way
                                });
                            data_tx.send(message::Request::Episode(item));
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
        app.display_mode = r.display_type();
        match r {
            message::Response::Feed(c) => {
                app.channel = Some(c);
            }
            message::Response::Episode(e) => {
                app.item = Some(e);
            }
        }
    }

    match app.display_mode {
        DisplayMode::EpisodeList => {
            display_feed_episodes(f, app, chunks[2], chunks[3]);
        }
        DisplayMode::ItemContent => {
            display_episode_details(f, app, chunks[2], chunks[3]);
        }
    };
}

#[tokio::main]
async fn handle_user_input(
    responder: &Sender<message::Response>,
    receiver: &Receiver<message::Request>,
) {
    if let Ok(r) = receiver.try_recv() {
        match r {
            message::Request::Feed(u) => {
                if let Ok(c) = feed::get_feed(u).await {
                    // TODO: error handling
                    responder.send(message::Response::Feed(c));
                }
            }
            message::Request::Episode(e) => {
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

    let podcast_name = Paragraph::new(app.channel.as_ref().map(|c| c.title()).unwrap_or("[Title]"));
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
    let p1 = Paragraph::new("EPISODE TITLE");
    let p2 = Paragraph::new("STUFF GOES HERE");
    f.render_widget(p1, title_area);
    f.render_widget(p2, content_area);
}
