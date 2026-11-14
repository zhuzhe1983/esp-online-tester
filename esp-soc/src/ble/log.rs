//! The virtual BLE controller's log lines: on stderr as `[ble] ...` for the CLI, or kept for a
//! host that has no stderr to read (the browser build takes them with `take`).
use std::collections::VecDeque;

/// The most lines kept; past it the oldest give way and are counted.
pub const KEPT_MAX: usize = 1024;

#[derive(Default)]
pub struct BleLog {
    kept: Option<VecDeque<String>>,
    dropped: u64,
}

impl BleLog {
    /// Keep the lines from now on instead of printing them.
    pub fn keep(&mut self) { self.kept.get_or_insert_with(VecDeque::new); }

    pub fn note(&mut self, line: String) {
        match &mut self.kept {
            Some(kept) => {
                if kept.len() >= KEPT_MAX { kept.pop_front(); self.dropped += 1; }
                kept.push_back(line);
            }
            None => eprintln!("[ble] {line}"),
        }
    }

    /// The oldest kept line. Lines lost to the bound come first as one `dropped=N` line.
    pub fn take(&mut self) -> Option<String> {
        if self.dropped > 0 { return Some(format!("dropped={}", std::mem::take(&mut self.dropped))); }
        self.kept.as_mut()?.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_kept_in_order_once_asked_and_the_overflow_is_counted() {
        let mut log = BleLog::default();
        log.note("to stderr".into());
        assert_eq!(log.take(), None, "nothing is kept before keep()");
        log.keep();
        for i in 0..KEPT_MAX + 2 { log.note(format!("line {i}")); }
        assert_eq!(log.take().as_deref(), Some("dropped=2"));
        assert_eq!(log.take().as_deref(), Some("line 2"));
        assert_eq!((0..KEPT_MAX - 1).filter_map(|_| log.take()).last().as_deref(), Some(format!("line {}", KEPT_MAX + 1).as_str()));
        assert_eq!(log.take(), None);
        log.keep();
        log.note("still kept".into());
        assert_eq!(log.take().as_deref(), Some("still kept"), "a second keep() keeps the queue");
    }
}
