use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;

/// Phase 3, Slice 1a: minimal scheduling-inspection panel.
///
/// This view is a stub on purpose — it proves the slash-command + panel
/// plumbing works (open from `/scheduling`, render, close on Esc) before we
/// wire the cross-process data fetch in Slice 1b. The panel currently shows
/// a single notice and closes on Esc / Enter / Ctrl-C. The real list of
/// cron / monitor / loop tasks will replace the notice in the next slice.
pub(crate) struct SchedulingView {
    complete: bool,
    header: Box<dyn Renderable>,
    body: Box<dyn Renderable>,
    footer_hint: Line<'static>,
}

impl SchedulingView {
    pub(crate) fn new() -> Self {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Scheduling tasks in this session".bold()));
        header.push(Line::from(
            "View active cron jobs, monitors, and loops scheduled in this session.".dim(),
        ));

        let mut body = ColumnRenderable::new();
        body.push(Line::from(""));
        body.push(Line::from(
            "Cron / Monitor / Loop inspection — data fetch wiring up next.".dim(),
        ));
        body.push(Line::from(
            "Use the agent in the meantime (e.g. \"show cron list\") to inspect.".dim(),
        ));

        Self {
            complete: false,
            header: Box::new(header),
            body: Box::new(body),
            footer_hint: scheduling_popup_hint_line(),
        }
    }
}

impl BottomPaneView for SchedulingView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.complete = true;
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for SchedulingView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let inner = content_area.inset(Insets::vh(/*v*/ 1, /*h*/ 2));
        let header_height = self.header.desired_height(inner.width);
        let body_height = self.body.desired_height(inner.width);
        let [header_area, _, body_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Max(body_height),
        ])
        .areas(inner);

        self.header.render(header_area, buf);
        self.body.render(body_area, buf);

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        // Add 1 for the footer hint row and 3 for vertical insets/spacer.
        let inner_width = width.saturating_sub(4);
        let header_height = self.header.desired_height(inner_width);
        let body_height = self.body.desired_height(inner_width);
        header_height
            .saturating_add(body_height)
            .saturating_add(4)
    }
}

fn scheduling_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " or ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to close".into(),
    ])
}
