//! The Remastered Credits screen: layout and scrolling of the CEF panel.
//!
//! The retail screen is the `webui/dist/creditspanel` page (Preact) over the
//! native glue backdrop.  Everything here is transcribed from that shipped
//! bundle (FACT) and checked against a 60 fps recording of the retail client
//! (2026-11-04, 3440x1440):
//!
//! * The CEF view is the centred 4:3 box (1920x1440 device px) at a device
//!   scale factor of 1.5, i.e. a 1280x960 CSS viewport, so the
//!   `(min-width:641px) and (max-width:1280px)` rules apply: `h1` 40 px
//!   Eurostile Extended (UA bold, `line-height: normal`, margins 0.67em),
//!   `h2` 28/30 px (margin-bottom 20), `h3` 22/24 px (margin 30 0 8), `h4`
//!   18/20 px (no margin), `h5` 14/16 px (margin-bottom 50).  The list box
//!   `.CreditsPanel` is `top:10.2%; margin-left:23%; width:57%; height:83%`,
//!   `overflow-y: scroll`, centred text, colour `#b4fc74`, six `0 0 1px`
//!   black text shadows.  Measured: every line's ink width matches the font
//!   advances at x1.5 within 1 px and the list is centred on x 1748.5 of
//!   the 3440 frame (predicted 1748.8).
//! * Vertical metrics are the Windows (DirectWrite) ones: OS/2 win
//!   ascent/descent plus the hhea-derived line gap.  The hhea-only (Linux)
//!   metrics put "1998" 6 px too high; these land on the recording.
//! * Scrolling is a `requestAnimationFrame` loop (`scroll()`): idle for
//!   `autoScrollDelay` = 2000 ms after mount, then `autoScrollSpeed` =
//!   `0.5 * floor(innerHeight / 480)` = 1 CSS px per frame (1.5 device px;
//!   measured 89.2-89.4 px/s, motion from 2.085 s after the screen
//!   appears).  Up/Down arrows start at `2.5k`, accelerate `0.5k` per frame
//!   up to `25k` and decelerate `k` per frame after release (`k =
//!   floor(innerHeight/480)` = 2); a key press or a wheel tick stops the
//!   auto scroll, which restarts 2000 ms later.  Esc and Enter send `Exit`.

/// CSS viewport of the credits CEF view.
pub const VIEW_CSS: (f32, f32) = (1280.0, 960.0);
/// `requestAnimationFrame` rate (frames per second).  Measured on the
/// recording: the list moves 1 CSS px (1.5 device px) per frame at 89.2 to
/// 89.4 px/s over 5.7 s (two independent fits), i.e. 59.5 frames per
/// second, a little under the nominal 60.
pub const FRAME_HZ: f32 = 59.5;
/// `autoScrollDelay` (ms), in animation frames.
pub const AUTO_DELAY_FRAMES: u32 = 120;
/// Frames between the screen appearing and the page's animation loop
/// starting.  Measured on the recording: the screen cuts in at 1.417 s and
/// the list moves from 3.502 s (fit over 10 frames, 89.2 px/s), 2.085 s
/// later against 2.0 s + one frame from the script alone.
pub const SHOWN_LATENCY_FRAMES: u64 = 4;
/// Mouse-wheel scroll per notch (CSS px).  INFERENCE: Chromium's default
/// 100 px per wheel tick; falsifier: a retail capture scrolling a notch.
pub const WHEEL_CSS: f32 = 100.0;

/// `floor(window.innerHeight / 480)`.
fn k() -> f32 {
    (VIEW_CSS.1 / 480.0).floor()
}

/// The panel's scroll state (`state` of the Preact component).
#[derive(Clone, Debug, PartialEq)]
pub struct CreditsScroll {
    /// `this.base.scrollTop`, CSS px.
    pub top: f32,
    value: f32,
    up: bool,
    down: bool,
    key_scrolling: bool,
    auto: bool,
    waiting: bool,
    /// Frames left on the `autoScrollTimeout`.
    timer: Option<u32>,
}

impl Default for CreditsScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl CreditsScroll {
    pub fn new() -> Self {
        Self {
            top: 0.0,
            value: 0.0,
            up: false,
            down: false,
            key_scrolling: false,
            auto: false,
            waiting: false,
            timer: None,
        }
    }

    fn auto_speed() -> f32 {
        0.5 * k()
    }

    /// `bounceAutoScroll`.
    fn bounce(&mut self) {
        self.auto = false;
        self.waiting = false;
        self.timer = None;
    }

    /// `handleKeydown` / `handleKeyup` for the arrow keys.
    pub fn set_keys(&mut self, up: bool, down: bool) {
        let pressed = (up && !self.up) || (down && !self.down);
        let released = (!up && self.up) || (!down && self.down);
        self.up = up;
        self.down = down;
        if pressed {
            self.key_scrolling = true;
            self.bounce();
        } else if released {
            self.key_scrolling = up || down;
        }
    }

    /// `mousewheel`: the browser scrolls the box and the handler stops the
    /// auto scroll (`notches` > 0 scrolls down).
    pub fn wheel(&mut self, notches: i32, max_top: f32) {
        if notches != 0 {
            self.top = (self.top + notches as f32 * WHEEL_CSS).clamp(0.0, max_top.max(0.0));
            self.bounce();
        }
    }

    /// One animation frame of `scroll()`; `max_top` is `scrollHeight -
    /// clientHeight` (the browser clamps `scrollTop`).
    pub fn frame(&mut self, max_top: f32) {
        // The pending setTimeout fires between frames.
        if let Some(t) = self.timer.as_mut() {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.timer = None;
                self.auto = true;
                self.value = Self::auto_speed();
            }
        }
        let (k, auto) = (k(), Self::auto_speed());
        let (initial, max, acc, dec) = (2.5 * k, 25.0 * k, 0.5 * k, k);
        if self.up {
            if self.value > -initial {
                self.value = -initial;
            } else if self.value > -max {
                self.value -= acc;
            }
        } else if self.down {
            if self.value < initial {
                self.value = initial;
            } else if self.value < max {
                self.value += acc;
            }
        } else if self.key_scrolling || self.auto {
            self.value = auto;
        } else {
            if !self.waiting {
                self.waiting = true;
                self.timer = Some(AUTO_DELAY_FRAMES);
            }
            if self.value > auto {
                self.value -= dec;
            } else if self.value < -auto {
                self.value += dec;
            } else {
                self.value = 0.0;
            }
        }
        self.top = (self.top + self.value).clamp(0.0, max_top.max(0.0));
    }
}

/// Vertical metrics of a face per em: (ascent, descent, line gap), the
/// DirectWrite way (OS/2 win ascent/descent; the gap is what the hhea line
/// spacing adds on top).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub gap: f32,
}

impl EmMetrics {
    pub fn from_font(bytes: &[u8]) -> Option<Self> {
        let face = ttf_parser::Face::parse(bytes, 0).ok()?;
        let em = face.units_per_em() as f32;
        let os2 = face.tables().os2?;
        let (wa, wd) = (
            os2.windows_ascender() as f32,
            os2.windows_descender().unsigned_abs() as f32,
        );
        let hhea = face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32;
        Some(Self {
            ascent: wa / em,
            descent: wd / em,
            gap: (hhea - wa - wd).max(0.0) / em,
        })
    }
}

/// One laid-out credit line, in device px relative to the top of the
/// scrolled content.
#[derive(Clone, Debug, PartialEq)]
pub struct CreditLine {
    pub text: String,
    /// Font size, device px.
    pub px: f32,
    /// Baseline, device px from the content top.
    pub baseline: f32,
    pub bold: bool,
    /// A horizontal rule (`style 6`) instead of text.
    pub rule: bool,
}

/// Credits laid out like the CEF page, in device px at `scale` device px per
/// CSS px (1.5 in the retail 1440p capture).
#[derive(Clone, Debug, PartialEq)]
pub struct CreditsLayout {
    pub lines: Vec<CreditLine>,
    /// `scrollHeight`, device px.
    pub height: f32,
}

/// Block geometry of one entry: (margin-top, margin-bottom, font px,
/// line-height or `None` for normal), CSS px.
fn style_box(style: u8) -> Option<(f32, f32, f32, Option<f32>)> {
    Some(match style {
        1 => (40.0 * 0.67, 40.0 * 0.67, 40.0, None),
        2 => (0.0, 20.0, 28.0, Some(30.0)),
        3 => (30.0, 8.0, 22.0, Some(24.0)),
        4 => (0.0, 0.0, 18.0, Some(20.0)),
        5 => (0.0, 50.0, 14.0, Some(16.0)),
        _ => return None,
    })
}

impl CreditsLayout {
    /// Lay out `entries` (`name`, `style`) from the bundle.  `title` are the
    /// metrics of Eurostile Extended (every heading), `body` those of
    /// Eurostile (the 16 px line box of a `style 6` rule).  Block margins
    /// collapse as in CSS (empty headings collapse through).
    pub fn new(entries: &[(String, u8)], title: EmMetrics, body: EmMetrics, scale: f32) -> Self {
        let mut y = 0.0f32;
        let mut pending = 0.0f32;
        let mut lines = Vec::new();
        // Blink with zoom-for-DSF lays out in device px and rounds the
        // font's ascent, descent and gap to whole pixels; a descent rounded
        // down borrows a pixel from the ascent, and the half leading is
        // floored into the ascent.  FACT (recording): this puts every
        // heading level on the retail baselines; without the borrow the
        // 33 px h3 sat 1.5 px low, without the floor h1/h2 0.5 px low.
        let line_box = |m: EmMetrics, px: f32, lh: Option<f32>| {
            let (mut a, mut d, g) = (
                (m.ascent * px).round(),
                (m.descent * px).round(),
                (m.gap * px).round(),
            );
            if d < m.descent * px && a >= 1.0 {
                d += 1.0;
                a -= 1.0;
            }
            let h = lh.unwrap_or(a + d + g);
            (h, a + ((h - (a + d)) / 2.0).floor())
        };
        for (name, style) in entries {
            if *style == 6 {
                let (h, base) = line_box(body, 16.0 * scale, None);
                y += pending;
                pending = 0.0;
                // vertical-align: middle -> centred on the x-height middle;
                // drawn at the baseline minus a quarter em.
                lines.push(CreditLine {
                    text: String::new(),
                    px: 16.0 * scale,
                    baseline: y + base - 4.0 * scale,
                    bold: false,
                    rule: true,
                });
                y += h;
                continue;
            }
            let Some((mt, mb, px, lh)) = style_box(*style) else {
                continue;
            };
            pending = pending.max(mt * scale);
            if name.is_empty() {
                // An empty heading has no line box: its margins collapse
                // through it into the next block.
                pending = pending.max(mb * scale);
                continue;
            }
            y += pending;
            let (h, base) = line_box(title, px * scale, lh.map(|l| l * scale));
            lines.push(CreditLine {
                text: name.clone(),
                px: px * scale,
                baseline: y + base,
                bold: *style == 1,
                rule: false,
            });
            y += h;
            pending = mb * scale;
        }
        Self {
            lines,
            height: y + pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_scroll_waits_two_seconds_then_moves_one_css_px_per_frame() {
        let mut s = CreditsScroll::new();
        for _ in 0..AUTO_DELAY_FRAMES {
            s.frame(10_000.0);
        }
        assert_eq!(s.top, 0.0, "idle for the 2000 ms autoScrollDelay");
        s.frame(10_000.0);
        assert_eq!(s.top, 1.0);
        for _ in 0..59 {
            s.frame(10_000.0);
        }
        // 1 CSS px per frame = 89.25 device px/s at x1.5 and 59.5 Hz.
        assert_eq!(s.top, 60.0);
        // The browser clamps at the end of the list.
        let mut end = s.clone();
        for _ in 0..100 {
            end.frame(70.0);
        }
        assert_eq!(end.top, 70.0);
    }

    #[test]
    fn arrow_keys_accelerate_and_pause_the_auto_scroll() {
        let mut s = CreditsScroll::new();
        s.set_keys(false, true);
        s.frame(1e6);
        assert_eq!(s.top, 5.0, "keyScrollInitialSpeed = 2.5 * 2");
        s.frame(1e6);
        assert_eq!(s.top, 11.0, "+ keyScrollAcceleration 0.5 * 2");
        for _ in 0..100 {
            s.frame(1e6);
        }
        let before = s.top;
        s.frame(1e6);
        assert_eq!(s.top - before, 50.0, "keyScrollMaxSpeed = 25 * 2");
        // Release: decelerate by 2 per frame down to the auto speed, then
        // stop; the auto scroll restarts 2000 ms later.
        s.set_keys(false, false);
        s.frame(1e6);
        assert_eq!(s.value, 48.0);
        for _ in 0..30 {
            s.frame(1e6);
        }
        assert_eq!(s.value, 0.0);
        let held = s.top;
        for _ in 0..(AUTO_DELAY_FRAMES - 31) {
            s.frame(1e6);
        }
        assert_eq!(s.top, held);
        for _ in 0..3 {
            s.frame(1e6);
        }
        assert!(s.top > held, "auto scroll resumed");
        // Up arrow scrolls back; the top clamps at 0.
        s.set_keys(true, false);
        for _ in 0..1000 {
            s.frame(1e6);
        }
        assert_eq!(s.top, 0.0);
    }

    #[test]
    fn wheel_scrolls_and_restarts_the_delay() {
        let mut s = CreditsScroll::new();
        for _ in 0..200 {
            s.frame(1e6);
        }
        let t = s.top;
        s.wheel(1, 1e6);
        assert_eq!(s.top, t + WHEEL_CSS);
        s.frame(1e6);
        assert_eq!(s.top, t + WHEEL_CSS, "auto scroll stopped");
    }

    #[test]
    fn layout_collapses_margins_and_uses_windows_metrics() {
        // Eurostile Extended: win 870/250, hhea 673/-327/200 -> gap 80.
        let ext = EmMetrics {
            ascent: 0.870,
            descent: 0.250,
            gap: 0.080,
        };
        let e = |s: &str, st: u8| (s.to_string(), st);
        let entries = [
            e("", 5),
            e("", 5),
            e("StarCraft", 1),
            e("1998", 2),
            e("", 5),
            e("Game Design", 3),
            e("Blizzard Entertainment", 4),
        ];
        let l = CreditsLayout::new(&entries, ext, ext, 1.5);
        let b: Vec<f32> = l.lines.iter().map(|x| x.baseline).collect();
        // h1 at the collapsed 75 px (two empty h5 margins beat its 40.2):
        // line 72 = 52 + 15 + 5, baseline 52 + floor(2.5).
        assert_eq!(b[0], 75.0 + 54.0);
        assert!(l.lines[0].bold && !l.lines[1].bold);
        // h2 after the h1's 40.2 bottom margin: 45 px line, 37 + 11 content,
        // half leading floor(-1.5).
        assert!((b[1] - (75.0 + 72.0 + 40.2 - 2.0 + 37.0)).abs() < 1e-3);
        // h3: the empty h5's 75 beats h2's 30 and h3's 45.
        let h2_end = 75.0 + 72.0 + 40.2 + 45.0;
        // h3 33 px: descent 8.25 rounds down to 8 and borrows -> 28 + 9,
        // half leading floor(-0.5) = -1.
        assert!((b[2] - (h2_end + 75.0 - 1.0 + 28.0)).abs() < 1e-3);
        assert!((b[3] - (h2_end + 75.0 + 36.0 + 12.0 + 23.0)).abs() < 1e-3);
    }
}
