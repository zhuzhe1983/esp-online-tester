//! Network monitoring: the Ethernet frames the WiFi station sends and receives (what the virtual
//! network, the NAT or the host relay sees), written to a classic pcap file (`--pcap`) and/or
//! streamed to the board page (`--web`). The station link (`wifi::StationLink`) records into a
//! shared `NetTap`; the timestamps are emulated time in microseconds.
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// The direction of a captured frame, as the websocket message carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir { FromStation = 0, ToStation = 1 }

/// The board page's binary message type for a captured frame: `[5][dir u8][ts_us u64 le][frame…]`.
pub const WS_NET_TYPE: u8 = 5;
/// The live backlog the page has not been sent yet: beyond either bound, new frames are dropped
/// (counted), so a busy network cannot grow memory while the emulator outruns the push interval.
const LIVE_FRAMES_MAX: usize = 512;
const LIVE_BYTES_MAX: usize = 1 << 20;
/// A pcap record keeps at most this much of a frame (the global header's snaplen).
const SNAPLEN: u32 = 65535;

/// The classic pcap global header: magic a1b2c3d4 (microsecond timestamps), version 2.4, UTC,
/// snaplen 65535, linktype 1 (Ethernet), little-endian.
pub fn pcap_global_header() -> [u8; 24] {
    let mut h = [0u8; 24];
    h[0..4].copy_from_slice(&0xa1b2_c3d4u32.to_le_bytes());
    h[4..6].copy_from_slice(&2u16.to_le_bytes());
    h[6..8].copy_from_slice(&4u16.to_le_bytes());
    h[16..20].copy_from_slice(&SNAPLEN.to_le_bytes());
    h[20..24].copy_from_slice(&1u32.to_le_bytes());
    h
}

/// One pcap record: header (seconds, microseconds, captured and original length) and the frame.
pub fn pcap_record(ts_us: u64, frame: &[u8]) -> Vec<u8> {
    let cap = frame.len().min(SNAPLEN as usize);
    let mut r = Vec::with_capacity(16 + cap);
    r.extend_from_slice(&((ts_us / 1_000_000) as u32).to_le_bytes());
    r.extend_from_slice(&((ts_us % 1_000_000) as u32).to_le_bytes());
    r.extend_from_slice(&(cap as u32).to_le_bytes());
    r.extend_from_slice(&(frame.len() as u32).to_le_bytes());
    r.extend_from_slice(&frame[..cap]);
    r
}

/// The board page's message for one frame.
pub fn ws_message(dir: Dir, ts_us: u64, frame: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(10 + frame.len());
    m.push(WS_NET_TYPE);
    m.push(dir as u8);
    m.extend_from_slice(&ts_us.to_le_bytes());
    m.extend_from_slice(frame);
    m
}

/// Where captured frames go: a pcap sink and/or the live queue for the board page.
#[derive(Default)]
pub struct NetCapture {
    pcap: Option<Box<dyn Write + Send>>,
    live: Option<VecDeque<Vec<u8>>>,
    live_bytes: usize,
    /// frames left out of the live queue because it was full
    pub live_dropped: u64,
    /// frames recorded
    pub frames: u64,
}

/// The capture the station link and the machine share.
pub type NetTap = Arc<Mutex<NetCapture>>;

impl NetCapture {
    /// Write a pcap to `sink`, starting with the global header now.
    pub fn set_pcap(&mut self, mut sink: Box<dyn Write + Send>) -> std::io::Result<()> {
        sink.write_all(&pcap_global_header())?;
        sink.flush()?;
        self.pcap = Some(sink);
        Ok(())
    }
    /// Keep frames for the board page (`take_live`).
    pub fn enable_live(&mut self) { self.live.get_or_insert_with(VecDeque::new); }
    pub fn live_enabled(&self) -> bool { self.live.is_some() }
    /// Record one frame. A pcap write error ends the pcap output, with one line saying so.
    pub fn record(&mut self, dir: Dir, ts_us: u64, frame: &[u8]) {
        self.frames += 1;
        if let Some(sink) = self.pcap.as_mut() {
            if let Err(e) = sink.write_all(&pcap_record(ts_us, frame)).and_then(|_| sink.flush()) {
                eprintln!("[emu] --pcap: {e}; capture stopped");
                self.pcap = None;
            }
        }
        if let Some(q) = self.live.as_mut() {
            let msg = ws_message(dir, ts_us, frame);
            if q.len() >= LIVE_FRAMES_MAX || self.live_bytes + msg.len() > LIVE_BYTES_MAX { self.live_dropped += 1; return; }
            self.live_bytes += msg.len();
            q.push_back(msg);
        }
    }
    /// The page messages queued since the last call.
    pub fn take_live(&mut self) -> Vec<Vec<u8>> {
        self.live_bytes = 0;
        self.live.as_mut().map(|q| q.drain(..).collect()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(b); Ok(b.len()) }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }

    #[test]
    fn pcap_has_global_header_and_a_record_per_frame_both_ways() {
        let buf = Buf::default();
        let mut cap = NetCapture::default();
        cap.set_pcap(Box::new(buf.clone())).unwrap();
        cap.record(Dir::FromStation, 1_500_000, &[0xaa; 60]);
        cap.record(Dir::ToStation, 2_000_007, &[0xbb; 42]);
        let b = buf.0.lock().unwrap().clone();
        assert_eq!(&b[..24], &[0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 1, 0, 0, 0]);
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        assert_eq!((u(24), u(28), u(32), u(36)), (1, 500_000, 60, 60));
        assert_eq!(&b[40..100], &[0xaa; 60]);
        assert_eq!((u(100), u(104), u(108), u(112)), (2, 7, 42, 42));
        assert_eq!(&b[116..], &[0xbb; 42]);
        assert_eq!(cap.frames, 2);
    }

    #[test]
    fn ws_message_layout_and_live_backlog_cap() {
        let m = ws_message(Dir::ToStation, 0x0102_0304_0506_0708, &[9, 8, 7]);
        assert_eq!(m, vec![5, 1, 8, 7, 6, 5, 4, 3, 2, 1, 9, 8, 7]);
        assert_eq!(ws_message(Dir::FromStation, 0, &[])[..2], [5, 0]);
        let mut cap = NetCapture::default();
        cap.record(Dir::FromStation, 0, &[1]);
        assert!(cap.take_live().is_empty(), "nothing kept until live is enabled");
        cap.enable_live();
        for i in 0..LIVE_FRAMES_MAX + 3 { cap.record(Dir::FromStation, i as u64, &[0; 64]); }
        assert_eq!(cap.live_dropped, 3);
        let live = cap.take_live();
        assert_eq!(live.len(), LIVE_FRAMES_MAX);
        assert_eq!(live[1][2..10], 1u64.to_le_bytes());
        cap.record(Dir::ToStation, 9, &[0; 64]);
        assert_eq!(cap.take_live().len(), 1, "room again after the queue was taken");
    }
}
