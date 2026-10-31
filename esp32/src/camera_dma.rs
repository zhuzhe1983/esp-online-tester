//! Scheduled classic camera transport. Uses the native I2S channel and its one
//! internal-DRAM adapter; no S3 GDMA registers or second descriptor walker.
use crate::{board::aithinker::{ARM_WINDOW, FRAME_CYCLES, VSYNC_CYCLES}, bus::SocBus, camera_fifo::CameraFifo};
use std::sync::Arc;

type Frame = Arc<Vec<u8>>;

/// Zero words appended after a JPEG frame's last word, given RXEOF_NUM in words.
/// The model clocks 0x00 after EOI (inferred, not a silicon or OV2640 timing claim)
/// until one word before an EOF boundary, and never ends inside the first EOF block.
/// esp32-camera v2.1.4 cam_hal.c:291-380 checks SOI on the first EOF block, copies
/// the unfinished block at VSYNC and searches EOI backwards (cam_hal.c:755), so the
/// copied block then holds no stale DMA bytes after EOI and fb->len is the host length.
pub(crate) fn jpeg_padding_words(words: usize, eof_words: usize) -> usize {
    const MAX_EOF_WORDS: usize = 8 * 1024 * 1024 / 4;
    if eof_words == 0 || eof_words > MAX_EOF_WORDS { return 0; }
    let at_least = words.max(eof_words);
    at_least.checked_add(eof_words - 1 - at_least % eof_words)
        .and_then(|total| total.checked_sub(words)).unwrap_or(0)
}

pub(crate) struct Capture {
    armed_at: u64,
    saw_vsync: bool,
    packed: Vec<u8>,
    pos: usize,
    frame_at: Option<u64>,
    qualified: bool,
    claim_pending: bool,
    // Cold capture-only cache: no new field in the bus or ordinary PCM path.
    inverted: Option<(Frame, u8, Frame)>,
}
impl Capture {
    pub(crate) fn qualified(&self) -> bool { self.qualified }
    pub(crate) fn armed(armed_at: u64) -> Self {
        Self { armed_at, saw_vsync: false, packed: Vec::new(), pos: 0, frame_at: None, qualified: false, claim_pending: false, inverted: None }
    }
    fn claim_first_word(&mut self, has_word: bool, edge: impl FnOnce() -> Option<u64>) -> bool {
        if !self.claim_pending || !has_word { return false; }
        self.claim_pending = false;
        edge() == self.frame_at
    }
    pub(crate) fn inverted_frame(&mut self, source: Frame, mask: u8) -> Frame {
        match &self.inverted {
            Some((old, old_mask, frame)) if *old_mask == mask && Arc::ptr_eq(old, &source) => return frame.clone(),
            _ => {}
        }
        let frame = Arc::new(source.iter().map(|byte| byte ^ mask).collect::<Vec<_>>());
        self.inverted = Some((source, mask, frame.clone()));
        frame
    }
}
impl SocBus {
    // Only entered behind the existing clock + running-DMA gate. Keeping this
    // out of line prevents camera packing from expanding the classic tick body.
    #[inline(never)]
    pub(crate) fn camera_dma_data(&mut self, _cycles: u64) -> Option<Vec<u8>> {
        let rx = self.periph.i2s[0].camera_rx()?;
        if !rx.started { return Some(Vec::new()); }
        let Some((width, height, wire)) = self.camera_routed_frame() else {
            // Recording a start is not yet a route-qualified capture. Preserve
            // C7's wait on initially unavailable routes, then make loss terminal
            // once this receiver has actually observed a valid source.
            if self.periph.i2s[0].camera.as_ref().is_some_and(|c| c.qualified) {
                self.periph.i2s[0].abort_camera_dma();
                self.irq_dirty = true;
            }
            return Some(Vec::new());
        };
        let high = self.periph.gpio.signal_input(191) == Some(true);
        let edge = self.board.camera_frame_start(false);
        let eof_words = self.periph.i2s[0].rx_eof_words() as usize;
        let capture = self.periph.i2s[0].camera.get_or_insert_with(|| Box::new(Capture::armed(self.cycles)));
        capture.qualified = true;
        let fresh = edge.is_some_and(|at| at <= capture.armed_at && capture.armed_at - at <= ARM_WINDOW);
        if high && (!capture.packed.is_empty() || !fresh) {
            // End the previous active interval at the next VSYNC. Flush its
            // final whole words before beginning a new frame. EOF is a native
            // chunk interrupt, never an invented whole-frame completion flag.
            let tail = capture.packed[capture.pos..].to_vec();
            capture.packed.clear(); capture.pos = 0; capture.frame_at = None; capture.claim_pending = false;
            capture.saw_vsync = true;
            return Some(tail);
        }
        if capture.packed.is_empty() {
            let Some(at) = edge.filter(|&at| capture.saw_vsync ||
                (at <= capture.armed_at && capture.armed_at - at <= ARM_WINDOW)) else { return Some(Vec::new()); };
            capture.saw_vsync = false;
            let Ok(mut fifo) = CameraFifo::new(rx.fifo_mode) else { return Some(Vec::new()) };
            if self.board.camera_frame_is_jpeg() {
                capture.claim_pending = true;
                fifo.push(&wire, &mut capture.packed);
                fifo.end_row(&mut capture.packed);
                let words = capture.packed.len() / 4;
                if let Some(size) = words.checked_add(jpeg_padding_words(words, eof_words)).and_then(|n| n.checked_mul(4)) {
                    capture.packed.resize(size, 0);
                }
                // JPEG leaves the edge unclaimed until its first delivered word
                // (below): an RX stop/reset/start inside the arm window before that
                // word takes this whole frame from SOI; a delivered word is never replayed.
            } else {
                let Some(row_bytes) = (width as usize).checked_mul(2).filter(|n| *n != 0) else { return Some(Vec::new()) };
                if wire.len() != row_bytes.saturating_mul(height as usize) { return Some(Vec::new()); }
                for row in wire.chunks_exact(row_bytes) { fifo.push(row, &mut capture.packed); fifo.end_row(&mut capture.packed); }
                // A consumed opportunity survives I2S stop/reset in the board, so a
                // second start cannot replay even the first word of this frame.
                self.board.camera_frame_start(true);
            }
            capture.frame_at = Some(at);
        }
        // Both pulse-armed and delayed starts use the physical falling edge.
        // Catch up on the first tick after start, independent of tick splitting.
        let elapsed = self.cycles.saturating_sub(capture.frame_at?).min(FRAME_CYCLES - VSYNC_CYCLES);
        // Approximate uniform word delivery over the existing 10Hz frame. This
        // is not PCLK timing. Whole words survive arbitrary guest tick splits.
        let words = capture.packed.len() / 4;
        let end = ((elapsed as u128 * words as u128) / u128::from(FRAME_CYCLES - VSYNC_CYCLES)) as usize * 4;
        // The first word claims this frame's own edge if it is still unclaimed (a
        // raw frame claimed it above); never a newer edge a long step has crossed.
        if capture.claim_first_word(end != 0, || self.board.camera_frame_start(false)) {
            self.board.camera_frame_start(true);
        }
        let data = capture.packed[capture.pos..end].to_vec();
        capture.pos = end;
        if high {
            capture.packed.clear(); capture.pos = 0; capture.frame_at = None; capture.claim_pending = false;
            capture.saw_vsync = true;
        }
        Some(data)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge() {
        let mut capture = super::Capture::armed(10);
        capture.frame_at = Some(20);
        assert!(!capture.claim_first_word(true, || Some(20)), "unarmed claim cannot consume an edge");
        capture.claim_pending = true;
        assert!(!capture.claim_first_word(false, || Some(20)), "empty delivery retains the claim");
        assert!(capture.claim_pending);
        assert!(capture.claim_first_word(true, || Some(20)), "first word claims its edge");
        assert!(!capture.claim_pending, "first word consumes the claim");
        assert!(!capture.claim_first_word(true, || Some(20)), "subsequent words cannot claim");
        capture.claim_pending = true;
        assert!(!capture.claim_first_word(true, || Some(21)), "newer edge must remain pending");
        assert!(!capture.claim_pending, "even a missed edge consumes the old claim");
    }
    #[test]
    fn jpeg_padding_rejects_unbounded_guest_eof() {
        for eof in [0x0400_0000_u32, u32::MAX] {
            assert_eq!(super::jpeg_padding_words(623, eof as usize), 0, "RXEOF_NUM={eof:#x}");
        }
        assert_eq!(super::jpeg_padding_words(usize::MAX, 3), 0);
        let limit = 8 * 1024 * 1024 / 4;
        assert_eq!(super::jpeg_padding_words(1, limit), 2 * limit - 2);
        assert_eq!(super::jpeg_padding_words(1, limit + 1), 0);
    }
    #[test]
    fn jpeg_padding_ends_one_word_before_an_eof_boundary_after_one_block() {
        // (words, RXEOF_NUM) -> padded total.
        for (words, eof, total) in [(1, 4, 7), (3, 4, 7), (4, 4, 7), (5, 4, 7), (6, 4, 7), (7, 4, 7),
            (8, 4, 11), (11, 4, 11), (12, 4, 15), (2489, 1024, 3071), (3071, 1024, 3071),
            (3072, 1024, 4095), (1023, 1024, 2047), (1024, 1024, 2047), (5, 1, 5), (0, 1, 1), (9, 0, 9)] {
            assert_eq!(words + super::jpeg_padding_words(words, eof), total, "{words} words, EOF every {eof}");
        }
    }
}
