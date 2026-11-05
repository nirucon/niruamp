//! One frame scheduler for the whole window.
//!
//! Every reason to draw another frame without user input is collected into
//! [`FrameInputs`] and reduced to a single deadline. `update` asks egui for
//! exactly that one repaint per frame, so independent timers can never stack
//! into a fast repaint loop. Messages from NPLAY wake the window on their own
//! (coalesced in the protocol reader) and are not scheduled here.
use std::time::{Duration, Instant};

/// Toasts are fully visible for this long, then fade out.
pub const TOAST_FADE_START: Duration = Duration::from_millis(4500);
/// Toasts are removed after this age.
pub const TOAST_LIFETIME: Duration = Duration::from_secs(6);
/// Frame interval while a toast fades (about 15 fps; a fade needs no more).
pub const TOAST_FADE_STEP: Duration = Duration::from_millis(66);
/// Deadlines this close count as reached: a frame that wakes slightly early
/// does the work instead of scheduling another frame for the remainder.
pub const EARLY_TOLERANCE: Duration = Duration::from_millis(4);
/// Loading indicator refresh.
pub const LOADING_STEP: Duration = Duration::from_millis(400);

#[derive(Clone, Debug)]
pub struct FrameInputs {
    pub now: Instant,
    /// Nothing of the window can be seen: hidden to the tray, minimized, or
    /// (when pausing is enabled) unmapped/fully obscured. No frame is scheduled.
    pub hidden: bool,
    /// Playback is advancing; the LED clock shows whole seconds.
    pub playing: bool,
    /// Current playback position in seconds, used to land on the next second.
    pub position: f64,
    pub loading: bool,
    /// When the spectrum next needs a frame (poll due or easing step), if at all.
    pub vis_next: Option<Instant>,
    /// Creation times of the visible toasts.
    pub toasts: Vec<Instant>,
    pub search_due: Option<Instant>,
    pub vis_retry_at: Option<Instant>,
    /// A screenshot was requested and needs frames until it is captured.
    pub screenshot: bool,
}

impl FrameInputs {
    pub fn idle(now: Instant) -> Self {
        Self {
            now,
            hidden: false,
            playing: false,
            position: 0.0,
            loading: false,
            vis_next: None,
            toasts: Vec::new(),
            search_due: None,
            vis_retry_at: None,
            screenshot: false,
        }
    }
}

/// Time until the next frame is needed, or `None` when the window can sleep
/// until the next input or backend message.
pub fn next_frame(i: &FrameInputs) -> Option<Duration> {
    if i.hidden {
        return None;
    }
    if i.screenshot {
        return Some(Duration::ZERO);
    }
    let mut next: Option<Duration> = None;
    let mut take = |d: Duration| {
        next = Some(next.map_or(d, |n: Duration| n.min(d)));
    };
    let until = |at: Instant| at.saturating_duration_since(i.now);
    if i.playing {
        // Land just after the next whole second so the LED flips on time.
        let frac = if i.position.is_finite() && i.position >= 0.0 {
            i.position.fract()
        } else {
            0.0
        };
        let secs = (1.0 - frac + 0.005).clamp(0.05, 1.0);
        take(Duration::from_secs_f64(secs));
    }
    if i.loading {
        take(LOADING_STEP);
    }
    if let Some(at) = i.vis_next {
        take(until(at));
    }
    for created in &i.toasts {
        let age = i.now.saturating_duration_since(*created);
        if age < TOAST_FADE_START {
            take(TOAST_FADE_START - age);
        } else if age < TOAST_LIFETIME {
            take(TOAST_FADE_STEP.min(TOAST_LIFETIME - age));
        } else {
            take(Duration::ZERO);
        }
    }
    if let Some(at) = i.search_due {
        take(until(at) + Duration::from_millis(10));
    }
    if let Some(at) = i.vis_retry_at {
        take(until(at));
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    #[test]
    fn idle_window_sleeps() {
        assert_eq!(next_frame(&FrameInputs::idle(Instant::now())), None);
    }

    #[test]
    fn hidden_window_never_schedules_even_with_work() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.hidden = true;
        i.playing = true;
        i.loading = true;
        i.vis_next = Some(now);
        i.toasts = vec![now];
        i.screenshot = true;
        assert_eq!(next_frame(&i), None);
    }

    #[test]
    fn playing_clock_lands_on_the_next_whole_second() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.playing = true;
        i.position = 12.25;
        let d = next_frame(&i).unwrap();
        assert!(d >= ms(750) && d <= ms(760), "{d:?}");
        i.position = 12.999;
        assert_eq!(next_frame(&i).unwrap(), ms(50));
        i.position = f64::NAN;
        assert_eq!(next_frame(&i).unwrap(), Duration::from_secs(1));
    }

    #[test]
    fn spectrum_deadline_wins_over_clock() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.playing = true;
        i.position = 1.0;
        i.vis_next = Some(now + ms(33));
        assert_eq!(next_frame(&i).unwrap(), ms(33));
        // An overdue spectrum frame is due now, never in the past.
        i.vis_next = Some(now - ms(5));
        assert_eq!(next_frame(&i).unwrap(), Duration::ZERO);
    }

    #[test]
    fn toasts_sleep_until_fade_then_fade_at_bounded_rate() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.toasts = vec![now - ms(1000)];
        assert_eq!(next_frame(&i).unwrap(), ms(3500));
        i.toasts = vec![now - ms(5000)];
        assert_eq!(next_frame(&i).unwrap(), TOAST_FADE_STEP);
        i.toasts = vec![now - ms(5980)];
        assert_eq!(next_frame(&i).unwrap(), ms(20));
        i.toasts = vec![now - ms(7000)];
        assert_eq!(next_frame(&i).unwrap(), Duration::ZERO);
    }

    #[test]
    fn search_debounce_and_retry_are_scheduled() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.search_due = Some(now + ms(200));
        assert_eq!(next_frame(&i).unwrap(), ms(210));
        i.vis_retry_at = Some(now + ms(100));
        assert_eq!(next_frame(&i).unwrap(), ms(100));
    }

    #[test]
    fn loading_and_screenshot() {
        let now = Instant::now();
        let mut i = FrameInputs::idle(now);
        i.loading = true;
        assert_eq!(next_frame(&i).unwrap(), LOADING_STEP);
        i.screenshot = true;
        assert_eq!(next_frame(&i).unwrap(), Duration::ZERO);
    }

    #[test]
    fn spectrum_at_30_fps_never_exceeds_31_frames_per_second() {
        // Simulate one second of a visible, playing window with the spectrum
        // easing every 33 ms: every frame asks for the next deadline once.
        let start = Instant::now();
        let mut t = start;
        let mut frames = 0;
        let mut last_vis = start;
        while t < start + Duration::from_secs(1) {
            frames += 1;
            let mut i = FrameInputs::idle(t);
            i.playing = true;
            i.position = (t - start).as_secs_f64();
            i.vis_next = Some(last_vis + ms(33));
            let d = next_frame(&i).unwrap();
            t += d.max(ms(1));
            if t >= last_vis + ms(33) {
                last_vis = t;
            }
        }
        assert!(frames <= 31, "{frames} frames");
    }
}
