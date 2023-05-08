pub mod input;

use tracing::{debug, span, trace, Level};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::{message::DisplayAction, App};

pub fn draw_main_layout<B>(f: &mut Frame<B>, app: &mut App)
where
    B: Backend,
{
    let span = span!(Level::TRACE, "render_main");
    let _entered = span.enter();

    // draw the top bar and the main display area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(2)
        .vertical_margin(1)
        .constraints(
            [ // TODO: figure out the minimum size requirements
                Constraint::Percentage(10),  // help text
                Constraint::Percentage(15),  // input box
                Constraint::Percentage(65), // output contents
                Constraint::Percentage(15),  // play bar
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_hint(f, app, chunks[0]);
    draw_input_box(f, app, chunks[1]);
    draw_display_area(f, app, chunks[2]);
    draw_playbar(f, app, chunks[3]);
}

pub fn draw_hint<B: Backend>(f: &mut Frame<B>, _app: &App, parent: Rect) {
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
    f.render_widget(help_message, parent);
}

pub fn draw_input_box<B: Backend>(f: &mut Frame<B>, app: &App, parent: Rect) {
    let input = Paragraph::new(app.input.as_ref())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, parent);

    // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
    f.set_cursor(
        // Put cursor past the end of the input text
        parent.x + app.input.width() as u16 + 1,
        // Move one line down, from the border to the input line
        parent.y + 1,
    );
}

pub fn draw_display_area<B: Backend>(f: &mut Frame<B>, app: &mut App, parent: Rect) {
    let span = span!(Level::TRACE, "render_display_area");
    let _entered = span.enter();

    match app.display_action {
        DisplayAction::ListEpisodes | DisplayAction::Input => draw_episode_list(f, app, parent),
        DisplayAction::DescribeEpisode => draw_episode_details(f, app, parent),
    }
}

pub fn draw_episode_list<B: Backend>(f: &mut Frame<B>, app: &mut App, parent: Rect) {
    let span = span!(Level::TRACE, "render_feed");
    let _entered = span.enter();
    trace!("rendering podcast episodes");
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

    let podcast_name = app
        .channel
        .as_ref()
        .map(|c| format!("[{}]", c.title()))
        .unwrap_or("[Title]".to_string());

    debug!(num_episodes = contents.len(), name = podcast_name);

    let contents = List::new(contents)
        .block(Block::default().borders(Borders::ALL).title(podcast_name))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(contents, parent, &mut app.state);
}

pub fn draw_episode_details<B: Backend>(f: &mut Frame<B>, app: &App, parent: Rect) {
    let span = span!(Level::TRACE, "render_episode");
    let _entered = span.enter();
    trace!("rendering episode details");

    let episode_name = app
        .item
        .as_ref()
        .and_then(|i| i.title())
        .map(|t| format!("[{}]", t))
        .unwrap_or("[Episode Title]".to_string());
    let description = app
        .item
        .as_ref()
        .and_then(|i| i.description())
        .unwrap_or("Description");
    let description = html2text::from_read(description.as_bytes(), parent.width.into());
    let audio_link = app
        .item
        .as_ref()
        .and_then(|i| i.enclosure.as_ref())
        .map(|e| e.url())
        .unwrap_or("[Audio URL]");

    let text = vec![
        Spans::from(Span::styled(
            audio_link,
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from(Span::raw("")),
        Spans::from(Span::raw(description)),
    ];

    let contents = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(episode_name));
    f.render_widget(contents, parent);
}

// TODO: make this an actual play bar
pub fn draw_playbar<B: Backend>(f: &mut Frame<B>, _app: &mut App, parent: Rect) {
    let text = Spans::from(Span::raw("This is the playbar"));
    let contents = Paragraph::new(text).block(Block::default().borders(Borders::all()));
    f.render_widget(contents, parent);
}
