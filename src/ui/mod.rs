use tracing::{span, Level, trace};
use tui::{Frame, backend::Backend, text::{Span, Spans}, style::{Modifier, Style}, widgets::{Paragraph, Wrap, Borders, Block}, layout::{Layout, Direction, Rect}};

use crate::App;

pub fn draw_main_layout<B>(f: &mut Frame<B>, app: &App)
where
  B: Backend,
{
    let span = span!(Level::TRACE, "render_main");
    let _entered = span.enter();
}

pub fn draw_episode<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    parent: Rect
) {
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

