//! Holding back frames the daemon captured halfway through a redraw.
//!
//! A program redrawing its whole screen marks the redraw with synchronized
//! output (DEC mode 2026), and Herdr's emulator waits for the end of it before
//! projecting a frame. Windows panes run behind ConPTY, which renders the
//! program's output itself and does not pass those marks on, so a frame can
//! land mid-redraw: for a moment an agent's pane shows its transcript being
//! reprinted, shifted and half drawn, before the finished screen replaces it.
//!
//! A frame that rewrites much of the screen while keeping the layout and
//! every pane's scroll position therefore waits until it has been the newest
//! frame for [`QUIET`]; a frame that replaces it in that time is judged the
//! same way. A burst that never quiets still shows its newest frame every
//! [`LIMIT`]. Typing, cursor movement, a wheel scroll, a resize, a split, and
//! a popup show at once.

use herdr_client::protocol::PaneSurfaceFrame;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// How long a rewriting frame must stand before it is shown. Mid-redraw
/// frames are replaced well within it.
pub(super) const QUIET: Duration = Duration::from_millis(40);
/// The longest a burst of rewriting frames keeps the screen unchanged.
pub(super) const LIMIT: Duration = Duration::from_millis(100);
/// Fewer changed rows than this never count as a rewrite, however small the
/// frame: an edited line, a status bar, and a spinner change only a few.
const MIN_ROWS: usize = 5;

#[derive(Default)]
pub(super) struct Settle {
    /// The rewriting frame waiting to be shown, and when it arrived.
    waiting: Option<(Arc<PaneSurfaceFrame>, Instant)>,
    /// When the screen began waiting, across every frame of a burst.
    since: Option<Instant>,
}

impl Settle {
    /// Whether `ready` may replace `shown` on screen at `now`.
    pub(super) fn admit(
        &mut self,
        shown: Option<&Arc<PaneSurfaceFrame>>,
        ready: &Arc<PaneSurfaceFrame>,
        now: Instant,
    ) -> bool {
        let Some(shown) = shown.filter(|shown| !Arc::ptr_eq(shown, ready)) else {
            self.clear();
            return true;
        };
        if !rewrites(shown, ready) {
            self.clear();
            return true;
        }
        let arrived = match &self.waiting {
            Some((waiting, arrived)) if Arc::ptr_eq(waiting, ready) => *arrived,
            _ => {
                self.waiting = Some((ready.clone(), now));
                now
            }
        };
        let since = *self.since.get_or_insert(now);
        if now.duration_since(arrived) >= QUIET || now.duration_since(since) >= LIMIT {
            self.clear();
            return true;
        }
        false
    }

    /// When the waiting frame is due, so the window draws again by then.
    pub(super) fn due(&self) -> Option<Instant> {
        let (_, arrived) = self.waiting.as_ref()?;
        let since = self.since?;
        Some((*arrived + QUIET).min(since + LIMIT))
    }

    pub(super) fn clear(&mut self) {
        self.waiting = None;
        self.since = None;
    }
}

/// Whether `next` rewrites much of `shown` in place: the same size, panes,
/// pane rectangles, scroll positions, screens, and popup, but many changed
/// rows. That is what a program redrawing its screen produces, and what
/// ConPTY can deliver half done.
fn rewrites(shown: &PaneSurfaceFrame, next: &PaneSurfaceFrame) -> bool {
    let same_layout = shown.boot_id == next.boot_id
        && shown.frame.width == next.frame.width
        && shown.frame.height == next.frame.height
        && shown.popup.is_some() == next.popup.is_some()
        && shown.panes.len() == next.panes.len()
        && shown.panes.iter().zip(&next.panes).all(|(a, b)| {
            a.pane_id == b.pane_id
                && a.rect == b.rect
                && a.alternate_screen_active == b.alternate_screen_active
                && a.scroll.map(|scroll| scroll.offset_from_bottom)
                    == b.scroll.map(|scroll| scroll.offset_from_bottom)
        });
    let width = usize::from(next.frame.width);
    if !same_layout || width == 0 {
        return false;
    }
    let rows = (usize::from(next.frame.height) / 4).max(MIN_ROWS);
    shown
        .frame
        .cells
        .chunks(width)
        .zip(next.frame.cells.chunks(width))
        .filter(|(a, b)| a != b)
        .nth(rows - 1)
        .is_some()
}

#[cfg(test)]
mod tests;
