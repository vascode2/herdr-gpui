#![allow(clippy::unwrap_used)]
use super::*;
use herdr_client::protocol::{
    CellData, FrameData, PaneSurfacePane, PaneSurfaceScrollMetrics, SurfaceRect,
};

const WIDTH: u16 = 4;
const HEIGHT: u16 = 24;

/// A frame whose row `y` reads `label(y)`, with one pane scrolled `offset`
/// rows into its history.
fn frame(label: impl Fn(u16) -> String, offset: Option<u64>) -> Arc<PaneSurfaceFrame> {
    let rect = SurfaceRect {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    Arc::new(PaneSurfaceFrame {
        boot_id: "boot".into(),
        projection_revision: 1,
        surface_revision: 1,
        frame: FrameData {
            cells: (0..HEIGHT)
                .flat_map(|y| {
                    let label = label(y);
                    (0..WIDTH).map(move |x| CellData {
                        symbol: if x == 0 { label.clone() } else { " ".into() },
                        fg: 0,
                        bg: 0,
                        modifier: 0,
                        skip: false,
                        hyperlink: None,
                    })
                })
                .collect(),
            width: WIDTH,
            height: HEIGHT,
            cursor: None,
            hyperlinks: vec![],
            graphics: vec![],
        },
        panes: offset
            .map(|offset| PaneSurfacePane {
                pane_id: "w1:p1".into(),
                content_revision: 1,
                rect,
                inner_rect: rect,
                scrollbar_rect: None,
                scroll: Some(PaneSurfaceScrollMetrics {
                    offset_from_bottom: offset,
                    max_offset_from_bottom: 1000,
                    viewport_rows: u64::from(HEIGHT),
                }),
                focused: true,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            })
            .into_iter()
            .collect(),
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    })
}

fn screen(prefix: &'static str) -> Arc<PaneSurfaceFrame> {
    frame(move |y| format!("{prefix}{y}"), Some(0))
}

#[test]
fn a_frame_captured_mid_redraw_never_shows_when_the_finished_screen_follows() {
    let mut settle = Settle::default();
    let now = Instant::now();
    let shown = screen("a");
    let mid_redraw = screen("m");
    assert!(!settle.admit(Some(&shown), &mid_redraw, now));
    assert_eq!(settle.due(), Some(now + QUIET));
    // The redraw finishes: the agent's screen again, one line further on.
    let finished = frame(
        |y| {
            if y == 23 {
                "new".into()
            } else {
                format!("a{y}")
            }
        },
        Some(0),
    );
    assert!(settle.admit(Some(&shown), &finished, now + Duration::from_millis(16)));
    assert_eq!(settle.due(), None);
}

#[test]
fn a_rewrite_that_stands_is_shown_once_quiet() {
    let mut settle = Settle::default();
    let now = Instant::now();
    let (shown, cleared) = (screen("a"), screen("b"));
    assert!(!settle.admit(Some(&shown), &cleared, now));
    assert!(!settle.admit(
        Some(&shown),
        &cleared,
        now + QUIET - Duration::from_millis(1)
    ));
    assert!(settle.admit(Some(&shown), &cleared, now + QUIET));
}

#[test]
fn a_burst_that_never_quiets_shows_its_newest_frame_within_the_limit() {
    let mut settle = Settle::default();
    let start = Instant::now();
    let shown = screen("a");
    let mut at = start;
    let mut admitted = None;
    for step in 0..10u32 {
        let next = frame(move |y| format!("{step}-{y}"), Some(0));
        if settle.admit(Some(&shown), &next, at) {
            admitted = Some(at);
            break;
        }
        // Each frame is replaced before it has stood for `QUIET`.
        at += QUIET / 2;
    }
    let admitted = admitted.unwrap();
    assert!(admitted >= start + LIMIT && admitted < start + LIMIT + QUIET / 2);
}

#[test]
fn edits_scrolls_and_layout_changes_show_at_once() {
    let now = Instant::now();
    let shown = screen("a");
    // A line edited, the status bar, a spinner: a few rows.
    let edited = frame(
        |y| {
            if y < 4 {
                format!("e{y}")
            } else {
                format!("a{y}")
            }
        },
        Some(0),
    );
    // The wheel scrolled the pane: every row moves, and so does the offset.
    let scrolled = frame(|y| format!("a{}", y + 1), Some(1));
    // Another pane layout, here none.
    let relaid = frame(|y| format!("b{y}"), None);
    for next in [edited, scrolled, relaid] {
        let mut settle = Settle::default();
        assert!(settle.admit(Some(&shown), &next, now));
        assert_eq!(settle.due(), None);
    }
    // Nothing shown yet, or the very frame shown.
    let mut settle = Settle::default();
    assert!(settle.admit(None, &shown, now));
    assert!(settle.admit(Some(&shown), &shown, now));
}

/// Only Windows holds frames back; see `Presentation::picture_at`.
#[cfg(windows)]
#[test]
fn the_painted_picture_trails_live_only_while_a_rewrite_settles() {
    use crate::{presentation::Presentation, state::ConnectionStatus, state::LiveState};
    let pane_less = |prefix: &'static str| -> Arc<PaneSurfaceFrame> {
        frame(move |y| format!("{prefix}{y}"), None)
    };
    let live = |surface: &Arc<PaneSurfaceFrame>| {
        let mut live = LiveState::default();
        live.status = ConnectionStatus::Connected;
        let mut snapshot = crate::sidebar::layout_tests::snapshot(1);
        snapshot.boot_id = "boot".into();
        snapshot.revision = 1;
        live.snapshot = Some(Arc::new(snapshot));
        live.surface = Some(surface.clone());
        live
    };
    let mut presentation = Presentation::default();
    let now = Instant::now();
    let first = pane_less("a");
    let shown = presentation.picture_at(&live(&first), now).unwrap().frame;
    assert!(Arc::ptr_eq(&shown, &first));

    let rewrite = pane_less("m");
    let shown = presentation.picture_at(&live(&rewrite), now).unwrap().frame;
    assert!(Arc::ptr_eq(&shown, &first), "the rewrite waits");
    assert_eq!(presentation.due(), Some(now + QUIET));
    let shown = presentation
        .picture_at(&live(&rewrite), now + QUIET)
        .unwrap()
        .frame;
    assert!(
        Arc::ptr_eq(&shown, &rewrite),
        "and is painted once it stands"
    );
    assert_eq!(presentation.due(), None);

    // A lost connection's dimmed picture gives way to a current frame at once.
    presentation.hold();
    presentation.resume();
    let current = pane_less("c");
    let shown = presentation
        .picture_at(&live(&current), now + QUIET)
        .unwrap()
        .frame;
    assert!(Arc::ptr_eq(&shown, &current));
}
