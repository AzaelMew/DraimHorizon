// A small profiler for the Baritone port (MIT). Sections of the port's per-tick and per-frame work
// are timed by name; once a second the totals of that second are written, busiest first, to
// `profile.txt` in the port's data folder (Baritone tab > Settings > Profile). Off by default: a
// section then costs one atomic load. No game types, so it is tested on its own.
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static ON: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
}
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub name: &'static str,
    pub calls: u32,
    pub total: Duration,
    pub max: Duration,
}

#[derive(Default)]
struct Window {
    since: Option<Instant>,
    entries: Vec<Entry>,
}
thread_local! {
    static WINDOW: RefCell<Window> = RefCell::new(Window::default());
}

/// Adds one measured call of `name` to the window.
pub fn record(name: &'static str, took: Duration) {
    WINDOW.with(|w| {
        let mut w = w.borrow_mut();
        w.since.get_or_insert_with(Instant::now);
        match w.entries.iter_mut().find(|e| e.name == name) {
            Some(e) => {
                e.calls += 1;
                e.total += took;
                e.max = e.max.max(took);
            }
            None => w.entries.push(Entry {
                name,
                calls: 1,
                total: took,
                max: took,
            }),
        }
    });
}

/// Runs `f` and, with the profiler on, times it as `name`.
#[inline]
pub fn time<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    if !enabled() {
        return f();
    }
    let begun = Instant::now();
    let out = f();
    record(name, begun.elapsed());
    out
}

/// The text of a window: seconds covered, then one line per section, the busiest first.
pub fn format(seconds: f64, entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| b.total.cmp(&a.total).then(a.name.cmp(b.name)));
    let mut out = format!("window {seconds:.2} s; per second: calls, total ms, mean us, max ms\n");
    let all: Duration = entries.iter().map(|e| e.total).sum();
    let per = |d: Duration| d.as_secs_f64() * 1000.0 / seconds.max(0.001);
    out.push_str(&format!(
        "{:<28} {:>9} {:>10.2}\n",
        "(all sections)",
        "",
        per(all)
    ));
    for e in sorted {
        let calls = f64::from(e.calls);
        out.push_str(&format!(
            "{:<28} {:>9.0} {:>10.2} {:>9.1} {:>8.2}\n",
            e.name,
            calls / seconds.max(0.001),
            per(e.total),
            e.total.as_secs_f64() * 1e6 / calls.max(1.0),
            e.max.as_secs_f64() * 1000.0
        ));
    }
    out
}

/// When a second has passed since the window began: its report, and the window starts again.
pub fn take_report(now: Instant) -> Option<String> {
    WINDOW.with(|w| {
        let mut w = w.borrow_mut();
        let since = w.since?;
        let seconds = now.duration_since(since).as_secs_f64();
        if seconds < 1.0 {
            return None;
        }
        let entries = std::mem::take(&mut w.entries);
        w.since = None;
        Some(format(seconds, &entries))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_sums_calls_keeps_the_max_and_lists_the_busiest_first() {
        WINDOW.with(|w| *w.borrow_mut() = Window::default());
        record("small", Duration::from_micros(100));
        record("big", Duration::from_millis(4));
        record("big", Duration::from_millis(6));
        record("small", Duration::from_micros(300));
        let entries = WINDOW.with(|w| w.borrow().entries.clone());
        let big = entries.iter().find(|e| e.name == "big").unwrap();
        assert_eq!(big.calls, 2);
        assert_eq!(big.total, Duration::from_millis(10));
        assert_eq!(big.max, Duration::from_millis(6));
        let text = format(2.0, &entries);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("window 2.00 s"));
        assert!(lines[1].contains("(all sections)"));
        // 10.4 ms over two seconds is 5.20 ms a second.
        assert!(lines[1].contains("5.20"), "{}", lines[1]);
        assert!(lines[2].starts_with("big") && lines[3].starts_with("small"));
        assert!(lines[2].contains("5.00"), "{}", lines[2]);
    }

    #[test]
    fn a_report_comes_once_a_second_and_the_window_starts_again() {
        WINDOW.with(|w| *w.borrow_mut() = Window::default());
        let start = Instant::now();
        assert!(take_report(start).is_none());
        record("tick", Duration::from_millis(1));
        assert!(take_report(Instant::now()).is_none());
        let later = Instant::now() + Duration::from_millis(1100);
        let report = take_report(later).expect("a second has passed");
        assert!(report.contains("tick"));
        assert!(take_report(later).is_none());
    }

    #[test]
    fn a_section_is_only_timed_while_the_profiler_is_on() {
        WINDOW.with(|w| *w.borrow_mut() = Window::default());
        set_enabled(false);
        assert_eq!(time("quiet", || 7), 7);
        assert!(WINDOW.with(|w| w.borrow().entries.is_empty()));
        set_enabled(true);
        assert_eq!(time("loud", || 8), 8);
        set_enabled(false);
        assert_eq!(WINDOW.with(|w| w.borrow().entries.len()), 1);
    }
}
