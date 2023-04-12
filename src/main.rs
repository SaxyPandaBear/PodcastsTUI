mod feed;
mod message;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rss::{Channel};
use std::{error::Error, io, sync::mpsc::{Sender, Receiver, TryRecvError}, time::Duration};
use std::sync::mpsc;
use std::thread;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, ListItem, List},
    Frame, Terminal, text::{Spans, Span, Text}, style::{Color, Style, Modifier},
};
use unicode_width::UnicodeWidthStr;

// App holds the state of the application
// TODO: persist application state about podcast that is loaded.
#[derive(Default)]
struct App {
    // Current value of the input box
    input: String,
    channel: Channel,
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
    thread::spawn(move || {
        loop {
            handle_user_input(&ui_tx, &data_rx);
            thread::sleep(Duration::new(1, 0));
        }
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App, data_tx: &Sender<message::Request>, ui_rx: &Receiver<message::Response>) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app, ui_rx))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    return Ok(())
                }
                KeyCode::Enter => {
                    // submit a message to data layer
                    let msg = app.input.drain(..).collect::<String>();
                    if let Ok(u) = url::Url::parse(msg.as_str()) {
                        // TODO: handle error
                        data_tx.send(message::Request::Feed(u));
                    }
                }
                KeyCode::Backspace => {
                    app.input.pop(); // delete from input text
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
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
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(80),
            ]
            .as_ref(),
        )
        .split(f.size());

    let (msg, style) = (
        vec![
            Span::raw("Press "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to stop editing, "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to record the message"),
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
                app.channel = c;
            }
        }
    }

    let contents = app
        .channel
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let content = vec![Spans::from(Span::raw(format!("{}: {}", idx, item.title.as_deref().unwrap_or("Title missing!"))))];
            ListItem::new(content)
        })
        .collect::<Vec<ListItem>>();

    let podcast_name = Paragraph::new(app.channel.title.as_ref());
    let contents = List::new(contents).block(Block::default().borders(Borders::ALL).title("Episodes"));

    f.render_widget(podcast_name, chunks[2]);
    f.render_widget(contents, chunks[3]);
}

#[tokio::main]
async fn handle_user_input(responder: &Sender<message::Response>, receiver: &Receiver<message::Request>) {
    if let Ok(r) = receiver.try_recv() {
        match r {
            message::Request::Feed(u) => {
                let res = feed::get_feed(u).await;
                match res {
                    Ok(c) => {
                        // TODO: error handling
                        responder.send(message::Response::Feed(c));
                    }
                    _ => {}
                }
            },
        }
    }
}
