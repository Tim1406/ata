use codex_protocol::protocol::SchedulingCronRow;
use codex_protocol::protocol::SchedulingLoopRow;
use codex_protocol::protocol::SchedulingMonitorRow;
use codex_protocol::protocol::SchedulingTasksSnapshotEvent;
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

/// Phase 3, Slice 1b: scheduling-inspection panel backed by a real snapshot.
///
/// The panel starts in a "loading" state, then re-renders once the host pushes
/// a [`SchedulingTasksSnapshotEvent`] in via [`Self::set_snapshot`]. The
/// snapshot arrives through `BottomPane::notify_scheduling_snapshot` after the
/// TUI dispatches `AppCommand::ListSchedulingTasks` and the session replies
/// over the app-server `scheduling/tasks/snapshot` notification.
pub(crate) struct SchedulingView {
    complete: bool,
    snapshot: Option<SchedulingTasksSnapshotEvent>,
    footer_hint: Line<'static>,
}

impl SchedulingView {
    pub(crate) fn new() -> Self {
        Self {
            complete: false,
            snapshot: None,
            footer_hint: scheduling_popup_hint_line(),
        }
    }

    fn set_snapshot(&mut self, snapshot: SchedulingTasksSnapshotEvent) {
        self.snapshot = Some(snapshot);
    }

    fn header(&self) -> Box<dyn Renderable> {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Scheduling tasks in this session".bold()));
        header.push(Line::from(
            "Active cron jobs, monitors, and loops for this thread.".dim(),
        ));
        Box::new(header)
    }

    fn body(&self) -> Box<dyn Renderable> {
        let mut body = ColumnRenderable::new();
        let Some(snapshot) = &self.snapshot else {
            body.push(Line::from(""));
            body.push(Line::from("Loading…".dim()));
            return Box::new(body);
        };

        if !snapshot.scheduling_enabled {
            body.push(Line::from(""));
            body.push(Line::from(
                "Scheduling is not enabled. Toggle it on in /experimental.".dim(),
            ));
            return Box::new(body);
        }

        body.push(Line::from(""));
        body.push(Line::from(format!("Cron ({})", snapshot.cron_jobs.len()).bold()));
        if snapshot.cron_jobs.is_empty() {
            body.push(Line::from("  (none)".dim()));
        } else {
            for row in &snapshot.cron_jobs {
                body.push(cron_row_line(row));
            }
        }

        body.push(Line::from(""));
        body.push(Line::from(format!("Monitors ({})", snapshot.monitors.len()).bold()));
        if snapshot.monitors.is_empty() {
            body.push(Line::from("  (none)".dim()));
        } else {
            for row in &snapshot.monitors {
                body.push(monitor_row_line(row));
            }
        }

        body.push(Line::from(""));
        body.push(Line::from(format!("Loops ({})", snapshot.loops.len()).bold()));
        if snapshot.loops.is_empty() {
            body.push(Line::from("  (none)".dim()));
        } else {
            for row in &snapshot.loops {
                body.push(loop_row_line(row));
            }
        }

        Box::new(body)
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

    fn handle_scheduling_snapshot(&mut self, snapshot: SchedulingTasksSnapshotEvent) {
        self.set_snapshot(snapshot);
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
        let header = self.header();
        let body = self.body();
        let header_height = header.desired_height(inner.width);
        let body_height = body.desired_height(inner.width);
        let [header_area, _, body_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Max(body_height),
        ])
        .areas(inner);

        header.render(header_area, buf);
        body.render(body_area, buf);

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let inner_width = width.saturating_sub(4);
        let header_height = self.header().desired_height(inner_width);
        let body_height = self.body().desired_height(inner_width);
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

fn cron_row_line(row: &SchedulingCronRow) -> Line<'static> {
    let prompt = truncate(&row.prompt, 40);
    let next = row.next_fire_at.as_deref().unwrap_or("—");
    Line::from(format!(
        "  {} [{}] {}  next: {}  fired: {}",
        row.task_id, row.status, prompt, next, row.fire_count
    ))
}

fn monitor_row_line(row: &SchedulingMonitorRow) -> Line<'static> {
    let cmd = truncate(&row.command, 60);
    Line::from(format!(
        "  {} [{}] {}  lines: {}",
        row.task_id, row.status, cmd, row.lines_emitted
    ))
}

fn loop_row_line(row: &SchedulingLoopRow) -> Line<'static> {
    let prompt = truncate(&row.prompt, 40);
    let interval = match row.interval_seconds {
        Some(s) => format!("{s}s"),
        None => "dynamic".to_string(),
    };
    Line::from(format!(
        "  {} [{}] {}  every {}  iter: {}",
        row.task_id, row.status, prompt, interval, row.iteration_count
    ))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_to_string(view: &SchedulingView, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| view.render(f.area(), f.buffer_mut()))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn loading_state_renders_placeholder() {
        let view = SchedulingView::new();
        let out = render_to_string(&view, 80, 10);
        assert!(out.contains("Scheduling tasks"), "header missing: {out}");
        assert!(out.contains("Loading"), "loading placeholder missing: {out}");
    }

    #[test]
    fn snapshot_renders_cron_monitor_and_loop_rows() {
        let mut view = SchedulingView::new();
        view.set_snapshot(SchedulingTasksSnapshotEvent {
            cron_jobs: vec![SchedulingCronRow {
                task_id: "c1".into(),
                cron_expr: "* * * * *".into(),
                prompt: "say hi".into(),
                status: "Scheduled".into(),
                fire_count: 3,
                last_fired_at: None,
                next_fire_at: Some("2026-05-12T12:00:00Z".into()),
            }],
            monitors: vec![SchedulingMonitorRow {
                task_id: "m1".into(),
                command: "tail -f log".into(),
                status: "Running".into(),
                lines_emitted: 42,
                started_at: None,
                stopped_at: None,
            }],
            loops: vec![SchedulingLoopRow {
                task_id: "l1".into(),
                prompt: "poll status".into(),
                interval_seconds: Some(5),
                status: "Running".into(),
                iteration_count: 7,
                last_iter_at: None,
                next_wakeup_at: None,
            }],
            scheduling_enabled: true,
        });
        let out = render_to_string(&view, 100, 20);
        assert!(out.contains("Cron (1)"), "cron header missing: {out}");
        assert!(out.contains("c1"), "cron row missing: {out}");
        assert!(out.contains("Monitors (1)"), "monitors header missing: {out}");
        assert!(out.contains("m1"), "monitor row missing: {out}");
        assert!(out.contains("Loops (1)"), "loops header missing: {out}");
        assert!(out.contains("l1"), "loop row missing: {out}");
        assert!(out.contains("every 5s"), "interval missing: {out}");
    }

    #[test]
    fn snapshot_disabled_shows_off_message() {
        let mut view = SchedulingView::new();
        view.set_snapshot(SchedulingTasksSnapshotEvent {
            cron_jobs: vec![],
            monitors: vec![],
            loops: vec![],
            scheduling_enabled: false,
        });
        let out = render_to_string(&view, 80, 12);
        assert!(out.contains("Scheduling is not enabled"), "{out}");
    }
}
