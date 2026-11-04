//! Physical-pixel popup placement and cancellation of delayed focus-loss work.
//! Kept independent of the desktop runtime so the edge cases run on every CI host.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    pub fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        i64::from(x) >= i64::from(self.x)
            && i64::from(x) < self.right()
            && i64::from(y) >= i64::from(self.y)
            && i64::from(y) < self.bottom()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// Infer the taskbar edge from the icon, not from the window's previous monitor.
/// The nearest monitor edge also covers auto-hidden taskbars and overflow icons.
fn edge(icon: Rect, monitor: Rect, work: Rect) -> Edge {
    let cx = i64::from(icon.x) + i64::from(icon.width) / 2;
    let cy = i64::from(icon.y) + i64::from(icon.height) / 2;
    if cy >= work.bottom() {
        Edge::Bottom
    } else if cy < i64::from(work.y) {
        Edge::Top
    } else if cx < i64::from(work.x) {
        Edge::Left
    } else if cx >= work.right() {
        Edge::Right
    } else {
        [
            ((monitor.bottom() - cy).abs(), Edge::Bottom),
            ((cy - i64::from(monitor.y)).abs(), Edge::Top),
            ((cx - i64::from(monitor.x)).abs(), Edge::Left),
            ((monitor.right() - cx).abs(), Edge::Right),
        ]
        .into_iter()
        .min_by_key(|(distance, _)| *distance)
        .unwrap()
        .1
    }
}

/// All arguments are physical pixels, including the DPI-scaled gap. The returned
/// outer frame fits the work area even when the configured minimum size does not.
pub fn place(icon: Rect, monitor: Rect, work: Rect, size: (u32, u32), gap: u32) -> Rect {
    let width = size.0.max(1).min(work.width.max(1));
    let height = size.1.max(1).min(work.height.max(1));
    let gap = i64::from(gap);
    let cx = i64::from(icon.x) + i64::from(icon.width) / 2;
    let cy = i64::from(icon.y) + i64::from(icon.height) / 2;
    let (x, y) = match edge(icon, monitor, work) {
        Edge::Bottom => (
            cx - i64::from(width) / 2,
            i64::from(icon.y) - gap - i64::from(height),
        ),
        Edge::Top => (cx - i64::from(width) / 2, icon.bottom() + gap),
        Edge::Left => (icon.right() + gap, cy - i64::from(height) / 2),
        Edge::Right => (
            i64::from(icon.x) - gap - i64::from(width),
            cy - i64::from(height) / 2,
        ),
    };
    Rect {
        x: x.clamp(i64::from(work.x), work.right() - i64::from(width)) as i32,
        y: y.clamp(i64::from(work.y), work.bottom() - i64::from(height)) as i32,
        width,
        height,
    }
}

/// A hide/show, focus return or tray gesture invalidates every older blur job.
#[derive(Default)]
pub struct Dismissal {
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissAction {
    Focused,
    Defer,
    Hide,
}

#[derive(Default)]
pub struct TrayGesture {
    ignore_release: bool,
}

impl TrayGesture {
    pub fn press(&mut self, double_click: bool) {
        self.ignore_release = double_click;
    }

    /// A Windows double-click's second release must not toggle a second time.
    pub fn release_toggles(&mut self) -> bool {
        !std::mem::take(&mut self.ignore_release)
    }
}

impl Dismissal {
    pub fn invalidate(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    pub fn is_current(&self, token: u64) -> bool {
        self.generation == token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn anchors_to_all_four_taskbar_edges() {
        let monitor = r(0, 0, 1920, 1080);
        for (icon, work, expected) in [
            (
                r(1800, 1044, 24, 24),
                r(0, 0, 1920, 1040),
                r(1440, 278, 480, 760),
            ),
            (
                r(1800, 12, 24, 24),
                r(0, 40, 1920, 1040),
                r(1440, 42, 480, 760),
            ),
            (
                r(12, 920, 24, 24),
                r(48, 0, 1872, 1080),
                r(48, 320, 480, 760),
            ),
            (
                r(1884, 920, 24, 24),
                r(0, 0, 1872, 1080),
                r(1392, 320, 480, 760),
            ),
        ] {
            assert_eq!(place(icon, monitor, work, (480, 760), 6), expected);
        }
    }

    #[test]
    fn mixed_dpi_and_negative_monitor_origins_stay_in_physical_pixels() {
        for scale in [1, 2, 3] {
            let monitor = r(
                -1920 * scale,
                -1080 * scale,
                1920 * scale as u32,
                1080 * scale as u32,
            );
            let work = r(monitor.x, monitor.y, monitor.width, 1040 * scale as u32);
            let icon = r(
                -120 * scale,
                -36 * scale,
                24 * scale as u32,
                24 * scale as u32,
            );
            let result = place(
                icon,
                monitor,
                work,
                (480 * scale as u32, 760 * scale as u32),
                6 * scale as u32,
            );
            assert_eq!(result.x, -480 * scale);
            assert_eq!(result.y, -802 * scale);
            assert_eq!(result.width, 480 * scale as u32);
        }
    }

    #[test]
    fn auto_hide_overflow_and_small_workareas_are_clamped() {
        let monitor = r(1920, -200, 1280, 800);
        for icon in [r(3130, 576, 24, 24), r(3060, 470, 24, 24)] {
            let result = place(icon, monitor, monitor, (600, 950), 9);
            assert_eq!(result.height, 800);
            assert_eq!(result.y, -200);
            assert!(result.x >= monitor.x && result.right() <= monitor.right());
        }
        let tiny = r(-300, 200, 280, 400);
        assert_eq!(place(r(-50, 602, 24, 24), tiny, tiny, (480, 760), 6), tiny);
    }

    #[test]
    fn fractional_dpi_and_reopening_on_another_monitor_have_no_old_position() {
        let first = place(
            r(1800, 1044, 24, 24),
            r(0, 0, 1920, 1080),
            r(0, 0, 1920, 1040),
            (480, 760),
            6,
        );
        let second = place(
            r(4300, 1400, 30, 30),
            r(1920, 0, 2560, 1440),
            r(1920, 0, 2560, 1390),
            (600, 950),
            8,
        );
        assert_eq!(first.width, 480);
        assert_eq!(second, r(3880, 440, 600, 950));
    }

    #[test]
    fn stale_blur_cannot_hide_a_reopened_or_refocused_panel() {
        let mut dismissal = Dismissal::default();
        let blur = dismissal.invalidate();
        assert!(dismissal.is_current(blur));
        dismissal.invalidate(); // tray press, explicit hide, show or focus return
        assert!(!dismissal.is_current(blur));
        let newer_blur = dismissal.invalidate();
        assert!(!dismissal.is_current(blur));
        assert!(dismissal.is_current(newer_blur));
    }

    #[test]
    fn double_click_toggles_once_and_next_single_click_still_toggles() {
        let mut gesture = TrayGesture::default();
        gesture.press(false);
        assert!(gesture.release_toggles());
        gesture.press(true);
        assert!(!gesture.release_toggles());
        gesture.press(false);
        assert!(gesture.release_toggles());
    }

    #[test]
    fn lost_release_does_not_poison_the_next_gesture() {
        let mut gesture = TrayGesture::default();
        gesture.press(true);
        gesture.press(false);
        assert!(gesture.release_toggles());
    }
}
