use crate::board::{BoardEdge, VirtualCycle};

/// Synthetic scan timing, not a calibration of a physical panel. The scan starts at
/// cycle zero and never restarts when STE or TE enable changes. All times are virtual cycles.
#[derive(Clone, Copy, Debug)]
pub struct Co5300Scan {
    pub lines: u16,
    pub period: VirtualCycle,
    pub pulse_width: VirtualCycle,
    pub pin: u8,
}

/// CO5300 QSPI command decoder and board-sized RGB565 frame memory.
pub struct Co5300 {
    width: usize,
    height: usize,
    column_offset: u16,
    pub frame: Vec<u16>,
    pub frames: u64,
    pub pixels_written: u64,
    x0: u16,
    x1: u16,
    y0: u16,
    y1: u16,
    x: u16,
    y: u16,
    pending: Option<u8>,
    args: [u8; 4],
    stream_header: [u8; 4],
    stream_count: usize,
    arg_count: usize,
    pixel_hi: Option<u8>,
    tear_row: u16,
    tear_enabled: bool,
    scan: Option<Co5300Scan>,
    cycle: VirtualCycle,
    next_rise: Option<VirtualCycle>,
    next_fall: Option<VirtualCycle>,
    tear_level: bool,
    edges: Vec<BoardEdge>,
}

impl Co5300 {
    pub fn new(width: usize, height: usize, column_offset: u16) -> Self {
        assert!(width > 0 && width <= 65536 - usize::from(column_offset));
        assert!(height > 0 && height <= 65536);
        Self {
            width,
            height,
            column_offset,
            frame: vec![0; width * height],
            frames: 0,
            pixels_written: 0,
            x0: column_offset,
            x1: (usize::from(column_offset) + width - 1) as u16,
            y0: 0,
            y1: (height - 1) as u16,
            x: 0,
            y: 0,
            pending: None,
            args: [0; 4],
            stream_header: [0; 4],
            stream_count: 0,
            arg_count: 0,
            pixel_hi: None,
            tear_row: 0,
            tear_enabled: false,
            scan: None,
            cycle: 0,
            next_rise: None,
            next_fall: None,
            tear_level: false,
            edges: Vec::new(),
        }
    }

    /// Opt into STE-aware TE edges. Without this, only command state is retained;
    /// boards with a legacy TE source can continue to own their waveform.
    pub fn with_scan(mut self, scan: Co5300Scan) -> Self {
        assert!(scan.lines > 0 && scan.period >= u64::from(scan.lines));
        assert!(scan.pulse_width > 0 && scan.pulse_width < scan.period);
        self.scan = Some(scan);
        self.schedule_rise();
        self
    }

    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
    pub fn tear_row(&self) -> u16 { self.tear_row }
    pub fn tear_enabled(&self) -> bool { self.tear_enabled }
    pub fn tear_input(&self) -> Option<(u8, bool)> { self.scan.map(|scan| (scan.pin, self.tear_level)) }

    /// Reset command state and RAM, retaining board geometry, scan parameters and phase.
    pub fn reset(&mut self) {
        self.disable_tear();
        let mut reset = Self::new(self.width, self.height, self.column_offset);
        reset.scan = self.scan;
        reset.cycle = self.cycle;
        reset.edges = std::mem::take(&mut self.edges);
        *self = reset;
    }

    fn schedule_rise(&mut self) {
        self.next_rise = None;
        let Some(scan) = self.scan else { return };
        if !self.tear_enabled || self.tear_row >= scan.lines { return; }
        let offset = (u128::from(self.tear_row) * u128::from(scan.period) / u128::from(scan.lines)) as u64;
        let at = (self.cycle / scan.period * scan.period).checked_add(offset);
        self.next_rise = at.and_then(|at| if at > self.cycle { Some(at) } else { at.checked_add(scan.period) });
    }

    fn disable_tear(&mut self) {
        self.tear_enabled = false;
        self.next_rise = None;
        self.next_fall = None;
        if self.tear_level {
            self.tear_level = false;
            if let Some(scan) = self.scan { self.edges.push(BoardEdge { cycle: self.cycle, pin: scan.pin, level: false }); }
        }
    }

    pub fn next_deadline(&self) -> Option<VirtualCycle> {
        match (self.next_rise, self.next_fall) {
            (Some(rise), Some(fall)) => Some(rise.min(fall)),
            (rise, fall) => rise.or(fall),
        }
    }

    pub fn advance_to(&mut self, cycle: VirtualCycle) {
        assert!(cycle >= self.cycle, "panel time moved backwards");
        while let Some(at) = self.next_deadline().filter(|&at| at <= cycle) {
            self.cycle = at;
            let scan = self.scan.expect("TE deadline requires scan timing");
            if self.next_fall == Some(at) {
                self.tear_level = false;
                self.next_fall = None;
                self.edges.push(BoardEdge { cycle: at, pin: scan.pin, level: false });
            }
            if self.next_rise == Some(at) {
                if !self.tear_level {
                    self.tear_level = true;
                    self.next_fall = at.checked_add(scan.pulse_width);
                    self.edges.push(BoardEdge { cycle: at, pin: scan.pin, level: true });
                }
                self.schedule_rise();
            }
        }
        self.cycle = cycle;
    }

    pub fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }

    pub fn transaction(&mut self, tx: &[u8]) {
        let data = if tx.len() >= 4 && matches!(tx[0], 0x02 | 0x32) {
            let command = tx[2];
            if command == 0x01 {
                self.reset();
                return;
            }
            self.pending = Some(command);
            self.arg_count = 0;
            self.pixel_hi = None;
            if command == 0x35 { self.tear_enabled = true; self.schedule_rise(); }
            if command == 0x34 { self.disable_tear(); }
            if command == 0x2c { self.x = self.x0; self.y = self.y0; self.frames += 1; }
            &tx[4..]
        } else { tx };
        self.transaction_data(data);
    }

    /// Begin a CS-delimited QSPI command. M5GFX 0.2.19 Panel_AMOLED::write_cmd
    /// sends its four header bytes in separate SPI transfers while CS stays low.
    pub fn begin_transaction(&mut self) {
        self.stream_count = 0;
        self.pending = None;
        self.pixel_hi = None;
    }

    /// Bytes under one asserted CS, including split headers and DMA chunks whose
    /// first pixel may look like a command. Existing transaction() callers keep
    /// their legacy complete-header convention.
    pub fn transaction_stream(&mut self, mut data: &[u8]) {
        if self.stream_count < 4 {
            let n = data.len().min(4 - self.stream_count);
            self.stream_header[self.stream_count..self.stream_count + n].copy_from_slice(&data[..n]);
            self.stream_count += n;
            data = &data[n..];
            if self.stream_count < 4 { return; }
            let header = self.stream_header;
            if matches!(header[0], 0x02 | 0x32) { self.transaction(&header); }
        }
        self.transaction_data(data);
    }

    fn transaction_data(&mut self, data: &[u8]) {
        match self.pending {
            Some(command @ (0x2a | 0x2b | 0x44)) => {
                let needed = if command == 0x44 { 2 } else { 4 };
                let n = data.len().min(needed - self.arg_count);
                self.args[self.arg_count..self.arg_count + n].copy_from_slice(&data[..n]);
                self.arg_count += n;
                if self.arg_count == needed {
                    let start = u16::from_be_bytes([self.args[0], self.args[1]]);
                    let end = u16::from_be_bytes([self.args[2], self.args[3]]);
                    match command {
                        0x2a => { self.x0 = start; self.x1 = end; self.x = start; }
                        0x2b => { self.y0 = start; self.y1 = end; self.y = start; }
                        _ => { self.tear_row = start; self.schedule_rise(); }
                    }
                    self.pending = None;
                }
            }
            Some(0x2c | 0x3c) => self.write_pixel_bytes(data),
            Some(_) => self.pending = None,
            None => {}
        }
    }

    /// EX158/EX170: the per-byte loop, a row run at a time. Same window walk and `pixels_written`.
    fn write_pixel_bytes(&mut self, mut data: &[u8]) {
        if let Some(high) = self.pixel_hi {
            let Some((&low, rest)) = data.split_first() else { return };
            self.pixel_hi = None;
            self.write_pixel(u16::from_be_bytes([high, low]));
            data = rest;
        }
        let (pairs, rest) = data.as_chunks::<2>();
        let mut pairs = pairs;
        while !pairs.is_empty() {
            let (x, x1) = (self.x as usize, self.x1 as usize);
            let to_edge = if x < x1 { x1 - x + 1 } else { 1 };
            let run = to_edge.min(pairs.len());
            let (now, later) = pairs.split_at(run);
            if (self.y as usize) < self.height {
                let lo = x.max(self.column_offset as usize);
                let hi = (x + run).min(self.column_offset as usize + self.width);
                if lo < hi {
                    let row = self.y as usize * self.width;
                    let xo = self.column_offset as usize;
                    for (dst, src) in self.frame[row + lo - xo..row + hi - xo].iter_mut().zip(&now[lo - x..hi - x]) { *dst = u16::from_be_bytes(*src); }
                    self.pixels_written += (hi - lo) as u64;
                }
            }
            if run == to_edge {
                self.x = self.x0;
                if self.y >= self.y1 { self.y = self.y0; } else { self.y += 1; }
            } else {
                self.x += run as u16;
            }
            pairs = later;
        }
        if let [byte] = rest { self.pixel_hi = Some(*byte); }
    }

    fn write_pixel(&mut self, pixel: u16) {
        let framebuffer_x = self.x.checked_sub(self.column_offset).map(usize::from);
        if framebuffer_x.is_some_and(|x| x < self.width) && (self.y as usize) < self.height {
            self.frame[self.y as usize * self.width + framebuffer_x.expect("checked panel column must exist")] = pixel;
            self.pixels_written += 1;
        }
        if self.x >= self.x1 {
            self.x = self.x0;
            if self.y >= self.y1 { self.y = self.y0; } else { self.y += 1; }
        } else {
            self.x += 1;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: usize = 368;
    const HEIGHT: usize = 448;
    const X_OFFSET: u16 = 16;

    fn panel() -> Co5300 { Co5300::new(WIDTH, HEIGHT, X_OFFSET) }

    fn command(panel: &mut Co5300, command: u8, params: &[u8]) {
        let mut tx = vec![0x02, 0, command, 0];
        tx.extend_from_slice(params);
        panel.transaction(&tx);
    }

    fn set_window(panel: &mut Co5300, x0: u16, x1: u16, y0: u16, y1: u16) {
        let mut columns = x0.to_be_bytes().to_vec();
        columns.extend_from_slice(&x1.to_be_bytes());
        let mut rows = y0.to_be_bytes().to_vec();
        rows.extend_from_slice(&y1.to_be_bytes());
        command(panel, 0x2a, &columns);
        command(panel, 0x2b, &rows);
    }

    #[test]
    fn window_boundaries_clip_at_both_board_geometries() {
        for (width, height, offset) in [(368, 448, 16), (468, 466, 6)] {
            let mut panel = Co5300::new(width, height, offset);
            assert_eq!((panel.width(), panel.height(), panel.frame.len()), (width, height, width * height));
            set_window(&mut panel, offset - 1, offset + width as u16, 0, height as u16);
            let mut bytes = Vec::new();
            for y in 0..=height {
                for x in 0..width + 2 {
                    bytes.extend_from_slice(&(0x8000 | ((y * (width + 2) + x) % 0x7000) as u16).to_be_bytes());
                }
            }
            command(&mut panel, 0x2c, &bytes);
            assert_eq!(panel.pixels_written, (width * height) as u64);
            for y in 0..height {
                for x in 0..width {
                    assert_eq!(panel.frame[y * width + x], 0x8000 | ((y * (width + 2) + x + 1) % 0x7000) as u16, "{width}x{height} at {x},{y}");
                }
            }
            assert_eq!((panel.x, panel.y), (offset - 1, 0));
        }
    }

    #[test]
    fn stopwatch_split_ramwr_ramwrc_strips_keep_the_468_column_stride() {
        let mut panel = Co5300::new(468, 466, 6);
        set_window(&mut panel, 6, 473, 0, 465);
        let expected: Vec<u16> = (0..466).flat_map(|y| (0..468).map(move |x| 0x8080 | ((y % 128) << 8) | (x % 128))).collect();
        for (strip, pixels) in expected.chunks(468 * 16).enumerate() {
            let bytes: Vec<u8> = pixels.iter().flat_map(|pixel| pixel.to_be_bytes()).collect();
            panel.transaction(&dma_transfer(if strip == 0 { 0x2c } else { 0x3c }, &bytes[..1]));
            panel.transaction(&bytes[1..937]);
            panel.transaction(&bytes[937..]);
            assert_eq!(panel.pixels_written as usize, ((strip + 1) * 468 * 16).min(468 * 466));
        }
        assert_eq!(panel.frame, expected);
        assert_eq!(panel.frames, 1);
        assert_eq!((panel.x, panel.y), (6, 0));
    }

    fn scan_panel() -> Co5300 {
        Co5300::new(468, 466, 6).with_scan(Co5300Scan { lines: 500, period: 15_000, pulse_width: 120, pin: 38 })
    }

    fn next_rise(panel: &mut Co5300) -> VirtualCycle {
        loop {
            let at = panel.next_deadline().expect("enabled scan has an edge");
            panel.advance_to(at);
            if let Some(edge) = panel.take_edges().into_iter().find(|edge| edge.level) { return edge.cycle; }
        }
    }

    #[test]
    fn ste_changes_mid_scan_select_the_next_crossing_without_restarting_phase() {
        let mut panel = scan_panel();
        panel.advance_to(3_000);
        command(&mut panel, 0x35, &[0]);
        command(&mut panel, 0x44, &320u16.to_be_bytes());
        assert_eq!(panel.next_deadline(), Some(9_600));
        panel.advance_to(6_000);
        command(&mut panel, 0x44, &100u16.to_be_bytes());
        assert_eq!(panel.next_deadline(), Some(18_000));
        panel.advance_to(8_000);
        command(&mut panel, 0x44, &320u16.to_be_bytes());
        panel.advance_to(9_599);
        assert!(panel.take_edges().is_empty());
        panel.advance_to(9_600);
        assert_eq!(panel.take_edges(), [BoardEdge { cycle: 9_600, pin: 38, level: true }]);
        command(&mut panel, 0x44, &0u16.to_be_bytes());
        assert_eq!(panel.next_deadline(), Some(9_720));
        panel.advance_to(15_000);
        assert_eq!(panel.take_edges(), [
            BoardEdge { cycle: 9_720, pin: 38, level: false },
            BoardEdge { cycle: 15_000, pin: 38, level: true },
        ]);
    }

    #[test]
    fn alternating_ste_zero_and_320_recovers_the_synthetic_scan() {
        let mut panel = scan_panel();
        command(&mut panel, 0x35, &[0]);
        command(&mut panel, 0x44, &0u16.to_be_bytes());
        next_rise(&mut panel);
        next_rise(&mut panel);
        let (mut forward, mut total) = (0, 0);
        for _ in 0..16 {
            let zero = next_rise(&mut panel);
            panel.advance_to(zero + 50);
            command(&mut panel, 0x44, &320u16.to_be_bytes());
            let line = next_rise(&mut panel);
            panel.advance_to(line + 50);
            command(&mut panel, 0x44, &0u16.to_be_bytes());
            let next = next_rise(&mut panel);
            assert_eq!((line - zero, next - line), (320 * 30, (500 - 320) * 30));
            forward += line - zero;
            total += next - zero;
        }
        let line_ns = forward * 1000 / (16 * 320);
        let lines = (total * 1000 / 16 + line_ns / 2) / line_ns;
        assert_eq!((line_ns, lines), (30_000, 500));
    }

    #[test]
    fn tear_commands_split_parameters_disable_and_reset_keep_scan_configuration() {
        let mut panel = scan_panel();
        assert_eq!(panel.tear_input(), Some((38, false)));
        assert!(!panel.tear_enabled());
        assert_eq!(panel.next_deadline(), None);
        command(&mut panel, 0x44, &[]);
        panel.transaction(&[1]);
        assert_eq!(panel.tear_row(), 0);
        panel.transaction(&[64]);
        assert_eq!(panel.tear_row(), 320);
        command(&mut panel, 0x35, &[0]);
        assert!(panel.tear_enabled());
        assert_eq!(next_rise(&mut panel), 9_600);
        panel.advance_to(9_650);
        command(&mut panel, 0x34, &[]);
        assert_eq!(panel.take_edges(), [BoardEdge { cycle: 9_650, pin: 38, level: false }]);
        assert_eq!(panel.next_deadline(), None);
        assert_eq!(panel.tear_row(), 320);
        panel.advance_to(16_000);
        command(&mut panel, 0x35, &[0]);
        assert_eq!(next_rise(&mut panel), 24_600);
        command(&mut panel, 0x01, &[]);
        assert_eq!(panel.take_edges(), [BoardEdge { cycle: 24_600, pin: 38, level: false }]);
        assert_eq!((panel.tear_enabled(), panel.tear_row(), panel.next_deadline()), (false, 0, None));
        assert_eq!((panel.width(), panel.height(), panel.x0, panel.x1, panel.y1), (468, 466, 6, 473, 465));
        command(&mut panel, 0x35, &[0]);
        assert_eq!(next_rise(&mut panel), 30_000);
    }

    #[test]
    fn scan_edges_keep_their_timestamps_when_advanced_in_one_batch() {
        let mut panel = scan_panel();
        command(&mut panel, 0x44, &320u16.to_be_bytes());
        command(&mut panel, 0x35, &[0]);
        panel.advance_to(24_720);
        assert_eq!(panel.take_edges(), [
            BoardEdge { cycle: 9_600, pin: 38, level: true },
            BoardEdge { cycle: 9_720, pin: 38, level: false },
            BoardEdge { cycle: 24_600, pin: 38, level: true },
            BoardEdge { cycle: 24_720, pin: 38, level: false },
        ]);
        command(&mut panel, 0x44, &500u16.to_be_bytes());
        assert_eq!(panel.next_deadline(), None);
        panel.advance_to(30_000);
        command(&mut panel, 0x44, &0u16.to_be_bytes());
        assert_eq!(panel.next_deadline(), Some(45_000));
    }

    #[test]
    fn scan_deadline_overflow_does_not_wrap_time() {
        let mut panel = scan_panel();
        panel.advance_to(u64::MAX - 100);
        command(&mut panel, 0x35, &[0]);
        if let Some(at) = panel.next_deadline() { panel.advance_to(at); }
        panel.advance_to(u64::MAX);
        assert_eq!(panel.next_deadline(), None);
    }

    fn cpu_transfer(command: u8, data: &[u8]) -> Vec<u8> {
        let mut spi = esp_periph::gpspi::GpSpi::new();
        spi.write(0x10, (1 << 31) | (1 << 30) | (1 << 27));
        spi.write(0x14, 23 << 27);
        spi.write(0x18, (7 << 28) | 0x02);
        spi.write(0x04, (command as u32) << 16);
        spi.write(0x1c, data.len() as u32 * 8 - 1);
        for (index, &byte) in data.iter().enumerate() {
            spi.w[index / 4] |= u32::from(byte) << (8 * (index % 4));
        }
        spi.write(0, 1 << 24);
        spi.take_transfer().expect("CPU GP-SPI transfer must be ready").tx
    }

    fn dma_transfer(command: u8, data: &[u8]) -> Vec<u8> {
        let mut spi = esp_periph::gpspi::GpSpi::new();
        spi.write(0x30, 1 << 28);
        spi.write(0x10, (1 << 31) | (1 << 30) | (1 << 27));
        spi.write(0x14, 23 << 27);
        spi.write(0x18, (7 << 28) | 0x32);
        spi.write(0x04, (command as u32) << 16);
        spi.write(0x1c, data.len() as u32 * 8 - 1);
        spi.write(0, 1 << 24);
        spi.complete_dma_tx(data);
        spi.take_transfer().expect("DMA GP-SPI transfer must be ready").tx
    }

    #[test]
    fn bulk_pixel_bytes_match_the_per_byte_walk() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = move |n: u32| { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed % n };
        for _ in 0..300 {
            let (mut a, mut b) = (panel(), panel());
            let x0 = rnd(400) as u16; let x1 = if rnd(8) == 0 { x0.saturating_sub(rnd(5) as u16) } else { x0.saturating_add(rnd(420) as u16) };   // also windows with x1 < x0
            let y0 = rnd(460) as u16; let y1 = y0 + rnd(30) as u16;
            for p in [&mut a, &mut b] {
                p.transaction(&[0x02, 0, 0x2a, 0, (x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8]);
                p.transaction(&[0x02, 0, 0x2b, 0, (y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8]);
                p.transaction(&[0x32, 0, 0x2c, 0]);
            }
            for _ in 0..rnd(6) + 1 {
                let mut data: Vec<u8> = (0..rnd(3000)).map(|_| rnd(256) as u8).collect();
                if let Some(first) = data.first_mut() { *first |= 0x80; } // not a 0x02/0x32 command header
                a.transaction(&data);
                for &byte in &data { match b.pixel_hi.take() { None => b.pixel_hi = Some(byte), Some(h) => b.write_pixel(u16::from_be_bytes([h, byte])) } }
            }
            assert_eq!((a.x, a.y, a.pixel_hi, a.pixels_written), (b.x, b.y, b.pixel_hi, b.pixels_written));
            assert!(a.frame == b.frame);
        }
    }

    #[test]
    fn combined_gpspi_parameter_and_color_phases_update_panel_pixels() {
        let mut panel = panel();
        panel.transaction(&cpu_transfer(0x2a, &[0, 0x11, 0, 0x12]));
        panel.transaction(&cpu_transfer(0x2b, &[0, 3, 0, 3]));
        panel.transaction(&dma_transfer(0x2c, &[0xf8, 0, 0x07, 0xe0]));
        assert_eq!(panel.frame[3 * WIDTH + 1], 0xf800);
        assert_eq!(panel.frame[3 * WIDTH + 2], 0x07e0);
        assert_eq!(panel.pixels_written, 2);
        assert_eq!(panel.frames, 1);
    }

    #[test]
    fn split_gpspi_commands_and_parameters_place_partial_updates_in_the_window() {
        let mut panel = panel();
        // The IDF LCD SPI driver sends these headers separately from the parameters.
        panel.transaction(&[0x02, 0, 0x2a, 0]);
        panel.transaction(&[0, 0x60, 0, 0x61]);
        panel.transaction(&[0x02, 0, 0x2b, 0]);
        panel.transaction(&[0, 140, 0, 141]);
        panel.transaction(&[0x32, 0, 0x2c, 0]);
        panel.transaction(&[0xf8, 0, 0x07, 0xe0]);
        panel.transaction(&[0x32, 0, 0x3c, 0]);
        panel.transaction(&[0, 0x1f, 0xff, 0xff]);
        assert_eq!(&panel.frame[140 * WIDTH + 80..140 * WIDTH + 82], &[0xf800, 0x07e0]);
        assert_eq!(&panel.frame[141 * WIDTH + 80..141 * WIDTH + 82], &[0x001f, 0xffff]);
        assert_eq!(panel.pixels_written, 4);
        assert_eq!(panel.frame[0], 0);
    }

    #[test]
    fn physical_x_offset_maps_the_full_panel_width_to_the_framebuffer() {
        let mut panel = panel();
        let x1 = X_OFFSET + WIDTH as u16 - 1;
        panel.transaction(&cpu_transfer(0x2a, &[0, X_OFFSET as u8, (x1 >> 8) as u8, x1 as u8]));
        panel.transaction(&cpu_transfer(0x2b, &[0, 0, 0, 0]));
        let mut pixels = Vec::with_capacity(WIDTH * 2);
        for pixel in 1..=WIDTH as u16 { pixels.extend_from_slice(&pixel.to_be_bytes()); }
        panel.transaction(&dma_transfer(0x2c, &pixels));
        assert_eq!(panel.pixels_written, WIDTH as u64);
        assert_eq!(panel.frame[0], 1);
        assert_eq!(panel.frame[WIDTH - 1], WIDTH as u16);
        assert_eq!(&panel.frame[..WIDTH], &(1..=WIDTH as u16).collect::<Vec<_>>());
    }

    #[test]
    fn software_reset_clears_the_panel_parser_window_and_framebuffer() {
        let mut panel = panel();
        panel.transaction(&cpu_transfer(0x2a, &[0, 0x11, 0, 0x11]));
        panel.transaction(&cpu_transfer(0x2b, &[0, 3, 0, 3]));
        panel.transaction(&dma_transfer(0x2c, &[0xf8, 0]));
        assert_eq!(panel.frame[3 * WIDTH + 1], 0xf800);

        panel.transaction(&[0x02, 0, 0x01, 0]);
        assert!(panel.frame.iter().all(|&pixel| pixel == 0));
        assert_eq!((panel.x0, panel.x1, panel.y0, panel.y1),
                   (X_OFFSET, X_OFFSET + WIDTH as u16 - 1, 0, HEIGHT as u16 - 1));
        assert_eq!((panel.pending, panel.pixel_hi, panel.frames, panel.pixels_written), (None, None, 0, 0));
    }

}
