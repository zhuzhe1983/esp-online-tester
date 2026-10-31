//! Tiny image loaders for camera sources: binary PPM (P6) and uncompressed 24/32-bit BMP.
//! Anything a page uploads comes through here, so a bad header is an error, never a panic.
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;
pub const MAX_RGBA_BYTES: u64 = 8 << 20;
pub struct Picture { pub w: u32, pub h: u32, pub rgb: Vec<u8> }

impl Picture {
    pub fn valid(&self) -> bool { rgba_len(self.w, self.h).is_some() && self.rgb.len() as u64 == self.w as u64 * self.h as u64 * 3 }
    fn pixel(&self, x: u32, y: u32, w: u32, h: u32) -> (u8, u8, u8) {
        let o = ((y as u64 * self.h as u64 / h as u64 * self.w as u64 + x as u64 * self.w as u64 / w as u64) * 3) as usize;
        (self.rgb[o], self.rgb[o + 1], self.rgb[o + 2])
    }
    pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Option<Self> {
        if rgba_len(w, h) != Some(rgba.len()) { return None; }
        let rgb = rgba.as_chunks::<4>().0.iter().flat_map(|p| p[..3].iter().copied()).collect();
        Some(Self { w, h, rgb })
    }
}

pub fn valid_size(w: u32, h: u32) -> bool { w != 0 && h != 0 && w as u64 * h as u64 <= MAX_PIXELS }
pub fn rgba_len(w: u32, h: u32) -> Option<usize> {
    if !valid_size(w, h) { return None; }
    let n = w as u64 * h as u64 * 4;
    (n + 5 <= MAX_RGBA_BYTES).then_some(n as usize)
}
fn luma(r: f32, g: f32, b: f32) -> f32 { 0.299 * r + 0.587 * g + 0.114 * b }

/// Baseline JPEG bytes the host has already encoded. Never decoded or re-encoded here:
/// a sensor in JPEG mode delivers exactly `bytes`. `w`/`h` are the SOF0 frame size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jpeg { pub w: u32, pub h: u32, pub bytes: std::sync::Arc<Vec<u8>> }

/// Why host JPEG input was refused; the previously supplied input is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JpegError {
    /// Invalid marker structure, missing SOF0 or scan, or EOI not last.
    Malformed,
    /// A frame header other than SOF0 (extended, progressive, lossless or arithmetic).
    NotBaseline,
    /// Larger than the 8 MiB host input limit.
    TooLarge,
    /// The SOF0 size differs from the sensor's configured output size.
    SizeMismatch { jpeg: (u32, u32), sensor: (u32, u32) },
    /// This board's camera does not accept encoded input.
    Unsupported,
}
impl std::fmt::Display for JpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => f.write_str("jpeg: malformed marker or segment structure (SOI, SOF0, SOS with entropy and final EOI required; no DNL; tables length-checked only, not decoded or semantically validated)"),
            Self::NotBaseline => f.write_str("jpeg: only baseline (SOF0) is supported"),
            Self::TooLarge => f.write_str("jpeg: input exceeds 8 MiB"),
            Self::SizeMismatch { jpeg, sensor } => write!(f, "jpeg: {}x{} does not match the sensor output {}x{}", jpeg.0, jpeg.1, sensor.0, sensor.1),
            Self::Unsupported => f.write_str("jpeg: this board's camera does not accept JPEG input"),
        }
    }
}
impl Jpeg {
    /// Check baseline marker and SOF0/SOS segment structure, taking the SOF0 size.
    /// A scan with entropy must follow SOF0 and EOI must end the input; DNL is refused.
    /// DHT and DQT contents are only length-checked. No decoding or semantic validation is done.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, JpegError> {
        if bytes.len() as u64 > MAX_RGBA_BYTES { return Err(JpegError::TooLarge); }
        // EOI must be the last marker and end the input: checked where the walk reaches it.
        if !bytes.starts_with(&[0xff, 0xd8]) { return Err(JpegError::Malformed); }
        let be16 = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize);
        let (mut pos, mut size, mut saw_scan) = (2, None, false);
        let mut components = Vec::new();
        loop {
            if bytes.get(pos) != Some(&0xff) { return Err(JpegError::Malformed); }
            while bytes.get(pos + 1) == Some(&0xff) { pos += 1; } // fill bytes before a marker
            let marker = *bytes.get(pos + 1).ok_or(JpegError::Malformed)?;
            pos += 2;
            match marker {
                0xd9 => return match size {
                    Some((w, h)) if saw_scan && pos == bytes.len() => Ok(Self { w, h, bytes: std::sync::Arc::new(bytes) }),
                    _ => Err(JpegError::Malformed),
                },
                0xc1..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => return Err(JpegError::NotBaseline),
                0xc0 | 0xc4 | 0xdb | 0xdd | 0xda | 0xe0..=0xef | 0xfe => {},
                _ => return Err(JpegError::Malformed),
            }
            let len = be16(pos).filter(|&n| n >= 2 && pos + n <= bytes.len()).ok_or(JpegError::Malformed)?;
            if marker == 0xc0 {
                if size.is_some() || len < 8 { return Err(JpegError::Malformed); }
                let (p, h, w) = (bytes[pos + 2], be16(pos + 3).unwrap_or(0) as u32, be16(pos + 5).unwrap_or(0) as u32);
                let n = bytes[pos + 7] as usize;
                if p != 8 || w == 0 || h == 0 || !(1..=4).contains(&n) || len != 8 + 3 * n { return Err(JpegError::Malformed); }
                for c in bytes[pos + 8..pos + len].as_chunks::<3>().0 {
                    let (hi, vi) = (c[1] >> 4, c[1] & 15);
                    if components.contains(&c[0]) || !(1..=4).contains(&hi) || !(1..=4).contains(&vi) || c[2] > 3 { return Err(JpegError::Malformed); }
                    components.push(c[0]);
                }
                size = Some((w, h));
            }
            if marker == 0xdd && len != 4 { return Err(JpegError::Malformed); }
            if marker == 0xda {
                if size.is_none() || len < 6 { return Err(JpegError::Malformed); }
                let n = bytes[pos + 2] as usize;
                if n == 0 || n > components.len() || len != 6 + 2 * n { return Err(JpegError::Malformed); }
                let mut seen = Vec::new();
                for c in bytes[pos + 3..pos + 3 + 2 * n].as_chunks::<2>().0 {
                    if !components.contains(&c[0]) || seen.contains(&c[0]) || c[1] >> 4 > 1 || c[1] & 15 > 1 { return Err(JpegError::Malformed); }
                    seen.push(c[0]);
                }
                if bytes[pos + 3 + 2 * n..pos + len] != [0, 63, 0] { return Err(JpegError::Malformed); }
            }
            pos += len;
            if marker == 0xda {
                saw_scan = true;
                // Entropy-coded data: FF 00 is a stuffed byte, FF D0..D7 a restart marker.
                let mut entropy = false;
                loop {
                    let n = bytes.get(pos..).and_then(|t| t.iter().position(|&b| b == 0xff)).ok_or(JpegError::Malformed)?;
                    entropy |= n != 0;
                    pos += n;
                    match bytes.get(pos + 1) {
                        Some(0x00) => { entropy = true; pos += 2; },
                        Some(0xd0..=0xd7) => pos += 2,
                        Some(_) if entropy => break,
                        Some(_) | None => return Err(JpegError::Malformed),
                    }
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat { Rgb565, Yuyv, Grayscale }

pub fn load(path: &str) -> Result<Picture, String> {
    let d = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    parse(&d).map_err(|e| format!("{}: {}", path, e))
}

/// Decode an image already in memory (the wasm build gets files from the page).
pub fn parse(d: &[u8]) -> Result<Picture, String> {
    if d.starts_with(b"P6") { return ppm(d); }
    if d.starts_with(b"BM") { return bmp(d); }
    Err("unsupported image (use PPM P6 or 24/32-bit BMP; `sips -s format bmp in.png --out out.bmp`)".into())
}

fn ppm(d: &[u8]) -> Result<Picture, String> {
    let mut i = 2; let mut nums = Vec::new();
    while nums.len() < 3 {
        while i < d.len() && (d[i] as char).is_whitespace() { i += 1; }
        if i < d.len() && d[i] == b'#' { while i < d.len() && d[i] != b'\n' { i += 1; } continue; }
        let s = i; while i < d.len() && d[i].is_ascii_digit() { i += 1; }
        if s == i { return Err("ppm: bad header".into()); }
        nums.push(std::str::from_utf8(&d[s..i]).unwrap().parse::<u32>().map_err(|e| e.to_string())?);
    }
    if !d.get(i).is_some_and(u8::is_ascii_whitespace) { return Err("ppm: missing pixel separator".into()); }
    i += 1;
    let (w, h) = (nums[0], nums[1]);
    if w == 0 || h == 0 || w as u64 * h as u64 > MAX_PIXELS { return Err(format!("ppm: unreasonable size {}x{}", w, h)); }
    let n = (w as u64 * h as u64 * 3) as usize;
    let rgb = d.get(i..).and_then(|tail| tail.get(..n)).ok_or("ppm: truncated")?.to_vec();
    Ok(Picture { w, h, rgb })
}

fn bmp(d: &[u8]) -> Result<Picture, String> {
    if d.len() < 54 { return Err("bmp: truncated header".into()); }
    let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let off = u32le(10) as usize; let w = u32le(18) as i32; let h = u32le(22) as i32; let bpp = u16::from_le_bytes([d[28], d[29]]) as usize;
    if bpp != 24 && bpp != 32 { return Err(format!("bmp: {} bpp not supported", bpp)); }
    let (wu, hu, flip) = (w.unsigned_abs(), h.unsigned_abs(), h > 0);
    if wu == 0 || hu == 0 || wu as u64 * hu as u64 > MAX_PIXELS { return Err(format!("bmp: unreasonable size {}x{}", wu, hu)); }
    let stride = ((wu as usize * bpp / 8) + 3) & !3;
    // Row padding separates rows; no decoder read needs padding after the final row.
    let len = stride.checked_mul(hu as usize - 1).and_then(|n| n.checked_add(wu as usize * bpp / 8)).ok_or("bmp: pixel extent overflows")?;
    let pixels = d.get(off..).and_then(|tail| tail.get(..len)).ok_or("bmp: truncated")?;
    let mut rgb = vec![0u8; (wu as u64 * hu as u64 * 3) as usize];
    for y in 0..hu as usize {
        let src_row = if flip { hu as usize - 1 - y } else { y };
        for x in 0..wu as usize {
            let p = src_row * stride + x * bpp / 8;
            let o = (y * wu as usize + x) * 3;
            rgb[o] = pixels[p + 2]; rgb[o + 1] = pixels[p + 1]; rgb[o + 2] = pixels[p];
        }
    }
    Ok(Picture { w: wu, h: hu, rgb })
}

/// Nearest-neighbour resample to `w`x`h` and pack as YUYV (BT.601 full range).
pub fn to_yuyv(p: &Picture, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 2) as usize];
    let yuv = |r: u8, g: u8, b: u8| -> (u8, u8, u8) {
        let (r, g, b) = (r as f32, g as f32, b as f32);
        let y = luma(r, g, b);
        let u = -0.169 * r - 0.331 * g + 0.5 * b + 128.0;
        let v = 0.5 * r - 0.419 * g - 0.081 * b + 128.0;
        (y.clamp(0.0, 255.0) as u8, u.clamp(0.0, 255.0) as u8, v.clamp(0.0, 255.0) as u8)
    };
    for y in 0..h {
        for x2 in 0..w / 2 {
            let mut px = [(0u8, 0u8, 0u8); 2];
            for (k, pixel) in px.iter_mut().enumerate() {
                let x = x2 * 2 + k as u32;
                let (r, g, b) = p.pixel(x, y, w, h);
                *pixel = yuv(r, g, b);
            }
            let o = ((y * w + x2 * 2) * 2) as usize;
            out[o] = px[0].0; out[o + 1] = ((px[0].1 as u16 + px[1].1 as u16) / 2) as u8; out[o + 2] = px[1].0; out[o + 3] = ((px[0].2 as u16 + px[1].2 as u16) / 2) as u8;
        }
    }
    out
}

/// Crop an inclusive sensor-array window, mapping its coordinates onto the host image.
/// Nearest-neighbour scaling models geometry, not the sensor's optical black pixels or ISP.
pub fn crop(p: &Picture, [x0, y0, x1, y1]: [u32; 4], array_w: u32, array_h: u32) -> Option<Picture> {
    if !p.valid() || x0 > x1 || y0 > y1 || x1 >= array_w || y1 >= array_h { return None; }
    let left = (x0 as u64 * p.w as u64 / array_w as u64) as u32;
    let top = (y0 as u64 * p.h as u64 / array_h as u64) as u32;
    let right = (((x1 + 1) as u64 * p.w as u64).div_ceil(array_w as u64) as u32).min(p.w);
    let bottom = (((y1 + 1) as u64 * p.h as u64).div_ceil(array_h as u64) as u32).min(p.h);
    let mut rgb = Vec::with_capacity(((right - left) * (bottom - top) * 3) as usize);
    for y in top..bottom {
        rgb.extend_from_slice(&p.rgb[((y * p.w + left) * 3) as usize..((y * p.w + right) * 3) as usize]);
    }
    Some(Picture { w: right - left, h: bottom - top, rgb })
}

/// Nearest-neighbour sampling; RGB565 bytes are most-significant first.
pub fn sensor_frame(p: &Picture, w: u32, h: u32, format: PixelFormat) -> Option<Vec<u8>> {
    if !p.valid() || !valid_size(w, h) { return None; }
    if matches!(format, PixelFormat::Yuyv) { return (w.is_multiple_of(2)).then(|| to_yuyv(p, w, h)); }
    let mut out = Vec::with_capacity((w * h * if matches!(format, PixelFormat::Grayscale) { 1 } else { 2 }) as usize);
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = p.pixel(x, y, w, h);
            if matches!(format, PixelFormat::Grayscale) {
                out.push(luma(r as f32, g as f32, b as f32).clamp(0.0, 255.0) as u8);
            } else {
                let rgb565 = ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3);
                out.extend_from_slice(&rgb565.to_be_bytes());
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_picture_shares_rgba_input_limit() {
        for (w, h, valid) in [(2048, 1023, true), (2048, 1024, false), (4096, 4096, false)] {
            let p = Picture { w, h, rgb: vec![0; (w * h * 3) as usize] };
            assert_eq!(p.valid(), valid);
            assert_eq!(rgba_len(w, h).is_some(), valid);
        }
    }
    #[test]
    fn formats_scaling_and_window() {
        let p = Picture { w: 2, h: 1, rgb: vec![255, 0, 0, 0, 255, 0] };
        assert_eq!(sensor_frame(&p, 2, 1, PixelFormat::Rgb565).unwrap(), [0xf8, 0, 0x07, 0xe0]);
        assert_eq!(sensor_frame(&p, 2, 1, PixelFormat::Grayscale).unwrap(), [76, 149]);
        assert_eq!(sensor_frame(&p, 2, 1, PixelFormat::Yuyv).unwrap(), [76, 63, 149, 138]);
        assert_eq!(sensor_frame(&p, 1, 2, PixelFormat::Rgb565).unwrap(), [0xf8, 0, 0xf8, 0]);
        let c = crop(&p, [1, 0, 1, 0], 2, 1).unwrap();
        assert_eq!(c.rgb, [0, 255, 0]);
        assert_eq!(sensor_frame(&c, 2, 1, PixelFormat::Rgb565).unwrap(), [7, 224, 7, 224]);
        assert!(crop(&p, [1, 0, 0, 0], 2, 1).is_none());
        assert!(sensor_frame(&p, 1, 1, PixelFormat::Yuyv).is_none());
        assert!(sensor_frame(&p, 0, 1, PixelFormat::Rgb565).is_none());
    }
    /// SOI, APP0, a COM segment holding FF D9, SOF0, SOS, entropy data with a stuffed
    /// byte and a restart marker, EOI. Structure only: tables are not needed to parse.
    fn jpeg(w: u16, h: u16, tag: u8) -> Vec<u8> {
        let mut d = vec![0xff, 0xd8, 0xff, 0xe0, 0, 4, b'J', b'F', 0xff, 0xfe, 0, 5, 0xff, 0xd9, tag];
        d.extend([0xff, 0xc0, 0, 11, 8]);
        d.extend(h.to_be_bytes()); d.extend(w.to_be_bytes());
        d.extend([1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0, tag, 0xff, 0, 0x12, 0xff, 0xd3, 0x34, 0xff, 0xd9]);
        d
    }
    #[test]
    fn jpeg_segment_validation() {
        let base = jpeg(640, 480, 7);
        let sof = base.windows(2).position(|w| w == [0xff, 0xc0]).unwrap();
        let sos = base.windows(2).position(|w| w == [0xff, 0xda]).unwrap();
        let cases: &[(&str, usize, u8)] = &[
            ("SOF length", sof + 3, 10), ("precision", sof + 4, 12),
            ("zero frame components", sof + 9, 0), ("too many frame components", sof + 9, 5),
            ("zero horizontal sampling", sof + 11, 0x01), ("high horizontal sampling", sof + 11, 0x51),
            ("zero vertical sampling", sof + 11, 0x10), ("high vertical sampling", sof + 11, 0x15),
            ("quantization table", sof + 12, 4), ("SOS length", sos + 3, 7),
            ("zero scan components", sos + 4, 0), ("too many scan components", sos + 4, 2),
            ("unknown scan component", sos + 5, 2), ("DC table", sos + 6, 0x20),
            ("AC table", sos + 6, 0x02), ("Ss", sos + 7, 1),
            ("Se", sos + 8, 62), ("Ah", sos + 9, 0x10), ("Al", sos + 9, 1),
        ];
        for &(name, offset, value) in cases {
            let mut input = base.clone(); input[offset] = value;
            assert_eq!(Jpeg::parse(input), Err(JpegError::Malformed), "{name}");
        }
        let mut excess_frame = base.clone();
        excess_frame[sof + 3] = 23; excess_frame[sof + 9] = 5;
        excess_frame.splice(sof + 13..sof + 13, [2, 0x11, 0, 3, 0x11, 0, 4, 0x11, 0, 5, 0x11, 0]);
        assert_eq!(Jpeg::parse(excess_frame), Err(JpegError::Malformed), "five frame components");
        let mut extra_frame_bytes = base.clone();
        extra_frame_bytes[sof + 3] = 14;
        extra_frame_bytes.splice(sof + 13..sof + 13, [2, 0x11, 0]);
        assert_eq!(Jpeg::parse(extra_frame_bytes), Err(JpegError::Malformed), "extra frame bytes");
        let mut empty_scan = base.clone();
        empty_scan[sos + 3] = 6; empty_scan[sos + 4] = 0;
        empty_scan.drain(sos + 5..sos + 7);
        assert_eq!(Jpeg::parse(empty_scan), Err(JpegError::Malformed), "zero scan components with matching length");
        for marker in [0, 1, 0xc8, 0xcc, 0xdc, 0xde, 0xdf, 0xf0, 0xd0, 0xd8] {
            let mut input = base.clone(); input.splice(sof..sof, [0xff, marker, 0, 2]);
            assert_eq!(Jpeg::parse(input), Err(JpegError::Malformed), "marker {marker:#x}");
        }
        for marker in 0xe0..=0xef {
            let mut input = base.clone(); input.splice(sof..sof, [0xff, marker, 0, 2]);
            assert!(Jpeg::parse(input).is_ok(), "APP {marker:#x}");
        }
        let mut input = base.clone(); input.splice(sof..sof, [0xff, 0xdd, 0, 4, 0, 1]);
        assert!(Jpeg::parse(input).is_ok(), "DRI length 4");
        for segment in [&[0xff, 0xdd, 0, 2][..], &[0xff, 0xdd, 0, 5, 0, 1, 2][..]] {
            let mut input = base.clone(); input.splice(sof..sof, segment.iter().copied());
            assert_eq!(Jpeg::parse(input), Err(JpegError::Malformed), "invalid DRI length");
        }
        let mut short_scan = base[..sos].to_vec(); short_scan.extend([0xff, 0xda, 0, 2]);
        let result = std::panic::catch_unwind(|| Jpeg::parse(short_scan));
        assert!(matches!(result, Ok(Err(JpegError::Malformed))), "truncated SOS must not panic");
        let mut two = base.clone();
        two[sof + 3] = 14; two[sof + 9] = 2;
        two.splice(sof + 13..sof + 13, [2, 0x11, 0]);
        let sos2 = sos + 3;
        two[sos2 + 3] = 10; two[sos2 + 4] = 2;
        two.splice(sos2 + 7..sos2 + 7, [2, 0]);
        assert!(Jpeg::parse(two.clone()).is_ok());
        let mut duplicate_frame = two.clone(); duplicate_frame[sof + 13] = 1;
        assert_eq!(Jpeg::parse(duplicate_frame), Err(JpegError::Malformed), "duplicate SOF component");
        let mut duplicate_unused = base.clone();
        duplicate_unused[sof + 3] = 14; duplicate_unused[sof + 9] = 2;
        duplicate_unused.splice(sof + 13..sof + 13, [1, 0x11, 0]);
        assert_eq!(Jpeg::parse(duplicate_unused), Err(JpegError::Malformed), "duplicate unused SOF component");
        let short_frame = vec![0xff, 0xd8, 0xff, 0xc0, 0, 6, 8, 0, 1, 0];
        let result = std::panic::catch_unwind(|| Jpeg::parse(short_frame));
        assert!(matches!(result, Ok(Err(JpegError::Malformed))), "truncated SOF must not panic");
        let mut duplicate_scan = two; duplicate_scan[sos2 + 7] = 1;
        assert_eq!(Jpeg::parse(duplicate_scan), Err(JpegError::Malformed), "duplicate SOS component");
    }
    #[test]
    fn jpeg_refuses_incomplete_sos_and_dnl() {
        let minimal = vec![0xff,0xd8, 0xff,0xc0,0,11,8,0,1,0,1,1,1,0x11,0, 0xff,0xda,0,2, 0xff,0xd9];
        let dnl = vec![0xff,0xd8, 0xff,0xc0,0,11,8,0,1,0,1,1,1,0x11,0, 0xff,0xda,0,8,1,1,0,0,63,0, 1, 0xff,0xdc,0,4,0,1, 0xff,0xd9];
        for (name, input) in [("empty SOS", minimal), ("DNL", dnl)] {
            assert_eq!(Jpeg::parse(input), Err(JpegError::Malformed), "{name}");
        }
    }
    #[test]
    fn jpeg_rejects_scan_without_entropy() {
        let input = vec![0xff,0xd8, 0xff,0xc0,0,11,8,0,1,0,1,1,1,0x11,0, 0xff,0xda,0,8,1,1,0,0,63,0, 0xff,0xd9];
        assert_eq!(Jpeg::parse(input.clone()), Err(JpegError::Malformed));
        let mut restart_only = input;
        let end = restart_only.len() - 2;
        restart_only.splice(end..end, [0xff, 0xd0]);
        assert_eq!(Jpeg::parse(restart_only), Err(JpegError::Malformed));
    }
    #[test]
    fn jpeg_requires_scan_after_sof0() {
        let no_scan = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 1, 0, 1, 1, 1, 0x11, 0, 0xff, 0xd9];
        assert_eq!(Jpeg::parse(no_scan), Err(JpegError::Malformed));
        let before_sof = vec![0xff, 0xd8, 0xff, 0xda, 0, 2, 0xff, 0xc0, 0, 8, 8, 0, 1, 0, 1, 1, 0xff, 0xd9];
        assert_eq!(Jpeg::parse(before_sof), Err(JpegError::Malformed));
    }
    #[test]
    fn jpeg_structure_and_sof0_size() {
        let good = jpeg(640, 480, 7);
        let parsed = Jpeg::parse(good.clone());
        assert!(parsed.is_ok(), "well-formed input: {parsed:?}");
        let j = parsed.unwrap();
        assert_eq!((j.w, j.h, &*j.bytes), (640, 480, &good));
        // Fill bytes before a marker, and a second scan after the first.
        let mut multi = good[..good.len() - 2].to_vec();
        multi.extend([0xff, 0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0, 9, 0xff, 0xd9]);
        assert_eq!(Jpeg::parse(multi).map(|j| (j.w, j.h)), Ok((640, 480)));
        let edit = |f: &dyn Fn(&mut Vec<u8>)| { let mut d = good.clone(); f(&mut d); Jpeg::parse(d) };
        let at = |marker: u8| good.windows(2).position(|w| w == [0xff, marker]).unwrap();
        for (what, result) in [
            ("no SOI", edit(&|d| d[1] = 0xd9)),
            ("no final EOI", edit(&|d| { d.pop(); })),
            ("bytes after EOI", edit(&|d| d.push(0))),
            ("segment past the end", edit(&|d| { let o = at(0xe0); d[o + 3] = 0xff; })),
            ("segment length below 2", edit(&|d| { let o = at(0xe0); d[o + 3] = 1; })),
            ("no marker after a segment", edit(&|d| { let o = at(0xfe); d[o + 7] = 0; })),
            ("12-bit SOF0", edit(&|d| { let o = at(0xc0); d[o + 4] = 12; })),
            ("zero width", edit(&|d| { let o = at(0xc0); d[o + 7] = 0; d[o + 8] = 0; })),
            ("zero height (DNL)", edit(&|d| { let o = at(0xc0); d[o + 5] = 0; d[o + 6] = 0; })),
            ("short SOF0", edit(&|d| { let o = at(0xc0); d[o + 3] = 7; })),
            ("SOF0 without components", edit(&|d| { let o = at(0xc0); d.drain(o + 9..o + 13); d[o + 3] = 7; })),
            ("no SOF0", edit(&|d| { let o = at(0xc0); d[o + 1] = 0xc4; })),
            ("SOF0 after the scan", edit(&|d| { let o = at(0xc0); let sof: Vec<u8> = d.drain(o..o + 13).collect(); let end = d.len() - 2; d.splice(end..end, sof); })),
            ("SOS length below 2", edit(&|d| { let o = at(0xda); d[o + 3] = 0; })),
            ("second SOF0", edit(&|d| { let o = at(0xda); let mut sof = d[at(0xc0)..at(0xc0) + 13].to_vec(); sof[10] = 2; d.splice(o..o, sof); })),
            ("EOI before the end", edit(&|d| { let o = at(0xda); d.splice(o..o, [0xff, 0xd9]); })),
            ("restart marker outside a scan", edit(&|d| { let o = at(0xc0); d.splice(o..o, [0xff, 0xd0, 0, 2]); })),
            ("second SOI", edit(&|d| { let o = at(0xc0); d.splice(o..o, [0xff, 0xd8, 0, 2]); })),
            ("TEM", edit(&|d| { let o = at(0xc0); d.splice(o..o, [0xff, 0x01, 0, 2]); })),
        ] {
            assert_eq!(result, Err(JpegError::Malformed), "{what}");
        }
        for marker in [0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf] {
            assert_eq!(edit(&|d| { let o = at(0xc0); d[o + 1] = marker; }), Err(JpegError::NotBaseline), "{marker:#x}");
        }
        assert_eq!(edit(&|d| { let o = at(0xc0); d.splice(o..o, [0xff, 0xc4, 0, 2]); }).map(|j| j.w), Ok(640));
        // Exactly 8 MiB is accepted, one byte more is not: full COM segments, then
        // fill bytes before the APP0 marker.
        let fill = MAX_RGBA_BYTES as usize - good.len();
        let mut pad = Vec::with_capacity(fill + 1);
        while fill - pad.len() >= 65537 { pad.extend([0xff, 0xfe, 0xff, 0xff]); pad.resize(pad.len() + 65533, 0); }
        pad.resize(fill, 0xff);
        let mut big = good.clone();
        big.splice(2..2, pad);
        assert_eq!(big.len() as u64, MAX_RGBA_BYTES);
        assert_eq!(Jpeg::parse(big.clone()).map(|j| (j.w, j.bytes.len() as u64)), Ok((640, MAX_RGBA_BYTES)));
        big.insert(2, 0xff);
        assert_eq!(Jpeg::parse(big), Err(JpegError::TooLarge));
        assert_eq!(JpegError::SizeMismatch { jpeg: (640, 480), sensor: (320, 240) }.to_string(), "jpeg: 640x480 does not match the sensor output 320x240");
    }
}
