//! ESP32-C61 ECC_MULT, independently mapped from ESP-IDF v5.5.4:
//! `components/soc/esp32c61/register/soc/ecc_mult_reg.h` and
//! `components/hal/esp32c61/include/hal/ecc_ll.h` (hardware modes 0, 2..11).
//! CONF mode is bits 4..7 and verification is bit 29, unlike C6.
//! Arithmetic uses the shared big-number primitives and the C6 Jacobian formulas.
//! Completion is synchronous; constant-time/clock bits are retained, not timed.
//! Jacobian outputs are mathematically equivalent, not guaranteed silicon bit-identical.
use esp_periph::{
    crypto::{bn_mod, bn_modexp, bn_mul},
    Device, WriteEffect,
};

type Number = Vec<u32>;

fn add(a: &[u32], b: &[u32], p: &[u32]) -> Number {
    let mut out = Vec::new();
    let mut carry = 0u64;
    for i in 0..a.len().max(b.len()) {
        let v =
            a.get(i).copied().unwrap_or(0) as u64 + b.get(i).copied().unwrap_or(0) as u64 + carry;
        out.push(v as u32);
        carry = v >> 32;
    }
    if carry != 0 {
        out.push(carry as u32);
    }
    bn_mod(&out, p)
}
fn sub(a: &[u32], b: &[u32], p: &[u32]) -> Number {
    let a = bn_mod(a, p);
    let b = bn_mod(b, p);
    let mut out = vec![0; p.len() + 1];
    let mut carry = 0u64;
    for i in 0..p.len() {
        let v = a.get(i).copied().unwrap_or(0) as u64 + p[i] as u64 + carry;
        out[i] = v as u32;
        carry = v >> 32;
    }
    out[p.len()] = carry as u32;
    let mut borrow = 0i64;
    for (i, word) in out.iter_mut().enumerate() {
        let v = *word as i64 - b.get(i).copied().unwrap_or(0) as i64 - borrow;
        *word = v as u32;
        borrow = i64::from(v < 0);
    }
    bn_mod(&out, p)
}
fn mul(a: &[u32], b: &[u32], p: &[u32]) -> Number {
    bn_mod(&bn_mul(a, b), p)
}
fn zero(a: &[u32]) -> bool {
    a.iter().all(|v| *v == 0)
}

struct Curve {
    p: Number,
    b: Number,
    order: Number,
}
impl Curve {
    fn new(p256: bool) -> Self {
        if !p256 {
            return Self {
                p: vec![0xffffffff, 0xffffffff, 0xfffffffe, 0xffffffff, 0xffffffff, 0xffffffff],
                b: vec![0xc146b9b1, 0xfeb8deec, 0x72243049, 0x0fa7e9ab, 0xe59c80e7, 0x64210519],
                order: vec![0xb4d22831, 0x146bc9b1, 0x99def836, 0xffffffff, 0xffffffff, 0xffffffff],
            };
        }
        Self {
            p: vec![0xffffffff, 0xffffffff, 0xffffffff, 0x00000000, 0x00000000, 0x00000000, 0x00000001, 0xffffffff],
            b: vec![0x27d2604b, 0x3bce3c3e, 0xcc53b0f6, 0x651d06b0, 0x769886bc, 0xb3ebbd55, 0xaa3a93e7, 0x5ac635d8],
            order: vec![0xfc632551, 0xf3b9cac2, 0xa7179e84, 0xbce6faad, 0xffffffff, 0xffffffff, 0x00000000, 0xffffffff],
        }
    }
    fn valid(&self, x: &[u32], y: &[u32]) -> bool {
        let p = &self.p;
        if !(less(x, p) && less(y, p)) {
            return false;
        }
        let rhs = add(
            &sub(&mul(&mul(x, x, p), x, p), &mul(x, &[3], p), p),
            &self.b,
            p,
        );
        zero(&sub(&mul(y, y, p), &rhs, p))
    }
}

struct Point {
    x: Number,
    y: Number,
    z: Number,
}
impl Point {
    fn infinity() -> Self {
        Self { x: vec![], y: vec![], z: vec![] }
    }
    fn double(&self, p: &[u32]) -> Self {
        if zero(&self.z) || zero(&self.y) {
            return Self::infinity();
        }
        let xx = mul(&self.x, &self.x, p);
        let yy = mul(&self.y, &self.y, p);
        let yyyy = mul(&yy, &yy, p);
        let zz = mul(&self.z, &self.z, p);
        let xy = add(&self.x, &yy, p);
        let s = mul(&sub(&sub(&mul(&xy, &xy, p), &xx, p), &yyyy, p), &[2], p);
        let m = mul(&sub(&xx, &mul(&zz, &zz, p), p), &[3], p);
        let x = sub(&mul(&m, &m, p), &mul(&s, &[2], p), p);
        let y = sub(&mul(&m, &sub(&s, &x, p), p), &mul(&yyyy, &[8], p), p);
        let yz = add(&self.y, &self.z, p);
        let z = sub(&sub(&mul(&yz, &yz, p), &yy, p), &zz, p);
        Self { x, y, z }
    }
    fn add_affine(&self, x: &[u32], y: &[u32], p: &[u32]) -> Self {
        if zero(&self.z) {
            return Self { x: x.to_vec(), y: y.to_vec(), z: vec![1] };
        }
        let zz = mul(&self.z, &self.z, p);
        let h = sub(&mul(x, &zz, p), &self.x, p);
        let r = sub(&mul(&mul(y, &self.z, p), &zz, p), &self.y, p);
        if zero(&h) {
            return if zero(&r) {
                self.double(p)
            } else {
                Self::infinity()
            };
        }
        let hh = mul(&h, &h, p);
        let hhh = mul(&h, &hh, p);
        let v = mul(&self.x, &hh, p);
        let nx = sub(&sub(&mul(&r, &r, p), &hhh, p), &mul(&v, &[2], p), p);
        let ny = sub(&mul(&r, &sub(&v, &nx, p), p), &mul(&self.y, &hhh, p), p);
        Self { x: nx, y: ny, z: mul(&self.z, &h, p) }
    }
    fn affine(&self, p: &[u32]) -> (Number, Number) {
        if zero(&self.z) {
            return (vec![], vec![]);
        }
        let inv = bn_modexp(&self.z, &sub(p, &[2], p), p);
        let inv2 = mul(&inv, &inv, p);
        (
            mul(&self.x, &inv2, p),
            mul(&mul(&self.y, &inv2, p), &inv, p),
        )
    }
}

// Numbers returned by bn_mod omit leading zero words; compare numerically.
fn less(a: &[u32], b: &[u32]) -> bool {
    (0..a.len().max(b.len())).rev().map(|i| a.get(i).copied().unwrap_or(0))
        .cmp((0..a.len().max(b.len())).rev().map(|i| b.get(i).copied().unwrap_or(0))).is_lt()
}

impl Curve {
    fn valid_jacobian(&self, point: &Point) -> bool {
        if zero(&point.z) || !less(&point.x, &self.p) || !less(&point.y, &self.p) || !less(&point.z, &self.p) { return false; }
        let p = &self.p;
        let zz = mul(&point.z, &point.z, p);
        let z4 = mul(&zz, &zz, p);
        let rhs = add(&sub(&mul(&mul(&point.x, &point.x, p), &point.x, p), &mul(&mul(&point.x, &z4, p), &[3], p), p), &mul(&self.b, &mul(&z4, &zz, p), p), p);
        zero(&sub(&mul(&point.y, &point.y, p), &rhs, p))
    }
    fn scale(&self, key: &[u32], x: &[u32], y: &[u32]) -> Point {
        let mut point = Point::infinity();
        for bit in (0..key.len() * 32).rev() {
            point = point.double(&self.p);
            if key[bit / 32] >> (bit % 32) & 1 != 0 { point = point.add_affine(x, y, &self.p); }
        }
        point
    }
}

/// C61 ECC: P-192/P-256 point operations and modular arithmetic, with six 32-byte banks.
pub struct Ecc {
    mem: [u32; 48],
    conf: u32,
    raw: u32,
    ena: u32,
    date: u32,
}
const VERIFIED: u32 = 1 << 29;
impl Default for Ecc {
    fn default() -> Self { Self { mem: [0; 48], conf: 0, raw: 0, ena: 0, date: 37752928 } }
}
impl Ecc {
    /// PCR ECC_RST_EN or CONF RESET clears registers and all parameter RAM.
    pub fn reset(&mut self) { *self = Self::default(); }
    fn store(&mut self, bank: usize, value: &[u32]) {
        self.mem[bank..bank + 8].fill(0);
        self.mem[bank..bank + value.len().min(8)].copy_from_slice(&value[..value.len().min(8)]);
    }
    fn verification(&mut self, valid: bool) { self.conf = (self.conf & !VERIFIED) | if valid { VERIFIED } else { 0 }; }
    fn calculate(&mut self) {
        let mode = (self.conf >> 4) & 15;
        self.raw = 0;
        // Reserved modes and division by zero remain incomplete instead of reporting a false result.
        if mode == 1 || mode > 11 { return; }
        let curve = Curve::new(self.conf & 4 != 0);
        let n = curve.p.len();
        let x = self.mem[8..8 + n].to_vec();
        let y = self.mem[16..16 + n].to_vec();
        match mode {
            0 | 3 | 4 | 7 => {
                let valid = curve.valid(&x, &y);
                if matches!(mode, 3 | 7) { self.verification(valid); }
                if matches!(mode, 0 | 4) || valid {
                    let point = curve.scale(&self.mem[..n], &x, &y);
                    if matches!(mode, 0 | 3) {
                        let (x, y) = point.affine(&curve.p);
                        self.store(8, &x); self.store(16, &y);
                    } else {
                        self.store(24, &point.x); self.store(32, &point.y); self.store(40, &point.z);
                    }
                }
            }
            2 => self.verification(curve.valid(&x, &y)),
            5 | 6 => {
                let point = Point { x: self.mem[24..24 + n].to_vec(), y: self.mem[32..32 + n].to_vec(), z: self.mem[40..40 + n].to_vec() };
                if mode == 6 { self.verification(curve.valid_jacobian(&point)); }
                else {
                    // IDF ecc_ll_set_mode maps POINT_ADD to 5 despite the register header's reserved label.
                    let result = point.add_affine(&x, &y, &curve.p);
                    let (x, y) = result.affine(&curve.p);
                    self.store(8, &x); self.store(16, &y);
                    self.store(24, &result.x); self.store(32, &result.y); self.store(40, &result.z);
                }
            }
            8..=11 => {
                let modulus = if self.conf & 8 != 0 { &curve.p } else { &curve.order };
                let result = match mode {
                    8 => add(&x, &y, modulus),
                    9 => sub(&x, &y, modulus),
                    10 => mul(&x, &y, modulus),
                    _ => {
                        if zero(&bn_mod(&y, modulus)) { return; }
                        // IDF test_ecc.c: write_mul(zero, numerator, denominator), result in PY.
                        mul(&x, &bn_modexp(&y, &sub(modulus, &[2], modulus), modulus), modulus)
                    }
                };
                self.store(if mode < 10 { 8 } else { 16 }, &result);
            }
            _ => unreachable!(),
        }
        self.conf &= !1;
        self.raw = 1;
    }
}
impl Device for Ecc {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0xc => self.raw,
            0x10 => self.raw & self.ena,
            0x14 => self.ena,
            0x1c => self.conf,
            0xfc => self.date,
            0x100..=0x1bc if off & 3 == 0 => self.mem[((off - 0x100) / 4) as usize],
            _ => 0,
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0xc | 0x18 => self.raw &= !(value & 1),
            0x14 => self.ena = value & 1,
            0x1c => {
                if value & 2 != 0 { self.reset(); }
                else {
                    self.conf = (value & 0xc00001fd) | (self.conf & VERIFIED);
                    if value & 1 != 0 { self.calculate(); }
                }
            }
            0xfc => self.date = value & 0x0fffffff,
            0x100..=0x1bc if off & 3 == 0 => self.mem[((off - 0x100) / 4) as usize] = value,
            _ => {}
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { (self.raw & self.ena) as u64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Numeric fixtures from ESP-IDF v5.5.4 hal/test_apps/crypto/main/ecc/ecc_params.h
    // (Espressif Systems 2023-2025, Apache-2.0), converted to little-endian u32 limbs.
    const P192_POINT_X: &[u32] = &[0x82ff1012, 0xf4ff0afd, 0x43a18800, 0x7cbf20eb, 0xb03090f6, 0x188da80e];
    const P192_POINT_Y: &[u32] = &[0x1e794811, 0x73f977a1, 0x6b24cdd5, 0x631011ed, 0xffc8da78, 0x07192b95];
    const P192_SCALAR: &[u32] = &[0x80348d5e, 0x7587bb6f, 0x02307048, 0x3c7771b3, 0x16b7ac9f, 0x6f1834eb];
    const P192_MUL_RES_X: &[u32] = &[0x6cc027c5, 0xec248237, 0xf5a0ace6, 0xb7471c2a, 0x99dccb78, 0x3fee6f1f];
    const P192_MUL_RES_Y: &[u32] = &[0x4654c3e1, 0xe8295ed9, 0x0bd2d983, 0xf00a18e1, 0x24f4f6b4, 0xdff39e76];
    const P192_JACOB_MUL_RES_X_LE: &[u32] = &[0xb23eb5e1, 0x2593ca85, 0xcf6bb1ca, 0x66b76ec3, 0x9883b49c, 0x55a17a8e];
    const P192_JACOB_MUL_RES_Y_LE: &[u32] = &[0x97ead3c7, 0xe5989287, 0xe4b130a5, 0x1171d992, 0x56376316, 0xb8d4b18d];
    const P192_JACOB_MUL_RES_Z_LE: &[u32] = &[0x768da5d1, 0x9fb37b17, 0x361b780d, 0xf1d01859, 0x13d78c38, 0xbf9b7acb];
    const P192_NUM: &[u32] = &[0xd82c0fba, 0xd32dccbe, 0x8cbd74d5, 0x7a3b3ef3, 0xec71d0a4, 0x0070f685];
    const P192_DEN: &[u32] = &[0xd820f915, 0x97035c46, 0x8aef104a, 0x652e12fb, 0x1e79d66e, 0x643e6f65];
    const P192_INV_MUL_RES: &[u32] = &[0x2b6bb36b, 0xf7e56a56, 0xccf08275, 0xf8406393, 0xaf2a35ef, 0x29e956bd];
    const P192_X: &[u32] = &[0x5fa1801a, 0x1b59b71f, 0xaefbd79f, 0xba1ef9a9, 0xb757ae67, 0x1a9e8027];
    const P192_Y: &[u32] = &[0xd33dc659, 0x44a3dfd7, 0xb452757c, 0xa6fcf342, 0x8d8aa80f, 0x54dfa31f];
    const P192_ADD_RES: &[u32] = &[0x32df4673, 0x5ffd96f7, 0x634e4d1b, 0x611becec, 0x44e25677, 0x6f7e2347];
    const P192_SUB_RES: &[u32] = &[0x4135e1f2, 0xeb21a0f9, 0x94885a58, 0x13220666, 0x29cd0658, 0xc5bedd08];
    const P192_MUL_RES: &[u32] = &[0xbcffb9b5, 0x36b8c852, 0xcea5fb8c, 0xf3e67b1e, 0xcf71798f, 0xe641f3d6];
    const P256_POINT_X: &[u32] = &[0xd898c296, 0xf4a13945, 0x2deb33a0, 0x77037d81, 0x63a440f2, 0xf8bce6e5, 0xe12c4247, 0x6b17d1f2];
    const P256_POINT_Y: &[u32] = &[0x37bf51f5, 0xcbb64068, 0x6b315ece, 0x2bce3357, 0x7c0f9e16, 0x8ee7eb4a, 0xfe1a7f9b, 0x4fe342e2];
    const P256_SCALAR: &[u32] = &[0x70112da2, 0xa9b47485, 0xe579cc73, 0xc860831e, 0x99611872, 0x9ec8836d, 0x64cd5f66, 0xb2c59e92];
    const P256_MUL_RES_X: &[u32] = &[0xf9a1e5e8, 0x9357b867, 0x765cd40e, 0x16694baf, 0xd14a7a2a, 0xb3c3b709, 0xa5e51ee7, 0x261a0fbd];
    const P256_MUL_RES_Y: &[u32] = &[0xa1c9d33a, 0xc7d40c90, 0xa7962924, 0x38f0eb6e, 0xd2958941, 0xf2a04142, 0x36259fe0, 0xa0f42e62];
    const P256_JACOB_MUL_RES_X_LE: &[u32] = &[0x1911799c, 0x51ab491b, 0x7b71575c, 0xa8d44a14, 0x7d0eea70, 0x0394203a, 0x952d9ea3, 0xb4d0c1ad];
    const P256_JACOB_MUL_RES_Y_LE: &[u32] = &[0x75ba46a4, 0xdbf21920, 0xe33f995d, 0x6efedf8c, 0x63631fd9, 0xafd66fdb, 0x4fed5ee8, 0x5a3c0599];
    const P256_JACOB_MUL_RES_Z_LE: &[u32] = &[0xbab47a7c, 0xeb86b7d4, 0xa10f79da, 0xab6aaba8, 0xbe05f9cb, 0x0e15905c, 0x0c6f2da7, 0x4abb68ae];
    const P256_NUM: &[u32] = &[0xb6145620, 0xb6a094af, 0x1a13df0c, 0x8757bfe6, 0x967302f1, 0xa9bc1a53, 0xfca15e0f, 0x9b9dfc0e];
    const P256_DEN: &[u32] = &[0x78113b54, 0xfd52cac4, 0x0f5189cc, 0x83377dfe, 0x582ed581, 0x194ff942, 0x9878799a, 0x2e4095fa];
    const P256_INV_MUL_RES: &[u32] = &[0x3b55f333, 0xc0138a46, 0xa6417e1d, 0x78fd53ff, 0x9fe5c0d5, 0x6686d178, 0xef6e3c77, 0x3429f658];
    const P256_X: &[u32] = &[0xd898c296, 0xf4a13945, 0x2deb33a0, 0x77037d81, 0x63a440f2, 0xf8bce6e5, 0xe12c4247, 0x6b17d1f2];
    const P256_Y: &[u32] = &[0x37bf51f5, 0xcbb64068, 0x6b315ece, 0x2bce3357, 0x7c0f9e16, 0x8ee7eb4a, 0xfe1a7f9b, 0x4fe342e2];
    const P256_ADD_RES: &[u32] = &[0x1058148b, 0xc05779ae, 0x991c926f, 0xa2d1b0d8, 0xdfb3df08, 0x87a4d22f, 0xdf46c1e3, 0xbafb14d5];
    const P256_SUB_RES: &[u32] = &[0xa0d970a1, 0x28eaf8dd, 0xc2b9d4d2, 0x4b354a29, 0xe794a2dc, 0x69d4fb9a, 0xe311c2ac, 0x1b348f0f];
    const P256_MUL_RES: &[u32] = &[0xccce4d18, 0x72eca81a, 0x41da31d7, 0xf16b758c, 0x535b2e2a, 0x6179ca8d, 0x2ef9466b, 0x1543b527];
    fn write(ecc: &mut Ecc, offset: u32, value: &[u32]) {
        for (i, word) in value.iter().enumerate() { ecc.write(offset + i as u32 * 4, *word); }
    }
    fn read(ecc: &mut Ecc, offset: u32, words: usize) -> Number {
        (0..words).map(|i| ecc.read(offset + i as u32 * 4)).collect()
    }
    fn start(ecc: &mut Ecc, mode: u32, p256: bool, prime: bool) {
        ecc.write(0x18, 1);
        ecc.write(0x1c, (mode << 4) | if p256 { 4 } else { 0 } | if prime { 8 } else { 0 } | 1);
        assert_eq!(ecc.read(0xc), 1);
        assert_eq!(ecc.read(0x1c) & 1, 0);
    }
    #[test]
    fn affine_scalar_vectors_from_idf_on_both_curves() {
        for (x, y, k, rx, ry) in [
            (P192_POINT_X, P192_POINT_Y, P192_SCALAR, P192_MUL_RES_X, P192_MUL_RES_Y),
            (P256_POINT_X, P256_POINT_Y, P256_SCALAR, P256_MUL_RES_X, P256_MUL_RES_Y),
        ] {
            for mode in [0, 3] {
                let mut ecc = Ecc::default();
                write(&mut ecc, 0x100, k); write(&mut ecc, 0x120, x); write(&mut ecc, 0x140, y);
                start(&mut ecc, mode, x.len() == 8, false);
                assert_eq!(read(&mut ecc, 0x120, x.len()), rx);
                assert_eq!(read(&mut ecc, 0x140, y.len()), ry);
                assert_eq!(ecc.read(0x1c) & VERIFIED != 0, mode == 3);
            }
        }
    }
    #[test]
    fn jacobian_vectors_verify_and_multiply_to_same_affine_point() {
        for (x, y, k, qx, qy, qz, rx, ry) in [
            (P192_POINT_X, P192_POINT_Y, P192_SCALAR, P192_JACOB_MUL_RES_X_LE, P192_JACOB_MUL_RES_Y_LE, P192_JACOB_MUL_RES_Z_LE, P192_MUL_RES_X, P192_MUL_RES_Y),
            (P256_POINT_X, P256_POINT_Y, P256_SCALAR, P256_JACOB_MUL_RES_X_LE, P256_JACOB_MUL_RES_Y_LE, P256_JACOB_MUL_RES_Z_LE, P256_MUL_RES_X, P256_MUL_RES_Y),
        ] {
            let mut ecc = Ecc::default();
            write(&mut ecc, 0x160, qx); write(&mut ecc, 0x180, qy); write(&mut ecc, 0x1a0, qz);
            start(&mut ecc, 6, x.len() == 8, false);
            assert_ne!(ecc.read(0x1c) & VERIFIED, 0);
            // Silicon's Jacobian representative is not unique. Compare affine coordinates.
            for mode in [4, 7] {
                write(&mut ecc, 0x100, k); write(&mut ecc, 0x120, x); write(&mut ecc, 0x140, y);
                start(&mut ecc, mode, x.len() == 8, false);
                let point = Point { x: read(&mut ecc, 0x160, x.len()), y: read(&mut ecc, 0x180, x.len()), z: read(&mut ecc, 0x1a0, x.len()) };
                let (ax, ay) = point.affine(&Curve::new(x.len() == 8).p);
                assert_eq!(ax, rx); assert_eq!(ay, ry);
            }
        }
    }
    #[test]
    fn modular_vectors_from_idf_use_px_py_and_curve_order() {
        for (x, y, sum, diff, product, num, den, quotient) in [
            (P192_X, P192_Y, P192_ADD_RES, P192_SUB_RES, P192_MUL_RES, P192_NUM, P192_DEN, P192_INV_MUL_RES),
            (P256_X, P256_Y, P256_ADD_RES, P256_SUB_RES, P256_MUL_RES, P256_NUM, P256_DEN, P256_INV_MUL_RES),
        ] {
            for (mode, a, b, expected) in [(8, x, y, sum), (9, x, y, diff), (10, x, y, product), (11, num, den, quotient)] {
                let mut ecc = Ecc::default();
                write(&mut ecc, 0x120, a); write(&mut ecc, 0x140, b);
                start(&mut ecc, mode, x.len() == 8, false);
                assert_eq!(read(&mut ecc, if mode < 10 { 0x120 } else { 0x140 }, x.len()), expected);
            }
        }
    }
    #[test]
    fn mod_prime_selection_wrap_and_divisor_zero_are_bounded() {
        for p256 in [false, true] {
            let curve = Curve::new(p256);
            for prime in [false, true] {
                let modulus = if prime { &curve.p } else { &curve.order };
                let mut ecc = Ecc::default();
                write(&mut ecc, 0x120, &sub(modulus, &[1], modulus)); write(&mut ecc, 0x140, &[2]);
                start(&mut ecc, 8, p256, prime);
                assert_eq!(ecc.read(0x120), 1);
                assert!(read(&mut ecc, 0x124, 7).iter().all(|v| *v == 0));
                write(&mut ecc, 0x120, &[0]);
                start(&mut ecc, 9, p256, prime);
                assert_eq!(read(&mut ecc, 0x120, modulus.len()), sub(modulus, &[2], modulus));
                ecc.store(16, &[]);
                ecc.write(0x1c, (11 << 4) | if p256 { 4 } else { 0 } | if prime { 8 } else { 0 } | 1);
                assert_eq!(ecc.read(0xc), 0);
                assert_eq!(ecc.read(0x1c) & 1, 1);
            }
        }
    }
    #[test]
    fn point_verification_rejects_invalid_noncanonical_and_infinity() {
        let mut ecc = Ecc::default();
        write(&mut ecc, 0x120, P256_POINT_X); write(&mut ecc, 0x140, P256_POINT_Y);
        start(&mut ecc, 2, true, false); assert_ne!(ecc.read(0x1c) & VERIFIED, 0);
        ecc.write(0x120, P256_POINT_X[0] ^ 1);
        let invalid = read(&mut ecc, 0x120, 8);
        start(&mut ecc, 3, true, false); assert_eq!(ecc.read(0x1c) & VERIFIED, 0);
        assert_eq!(read(&mut ecc, 0x120, 8), invalid);
        write(&mut ecc, 0x120, &Curve::new(true).p);
        start(&mut ecc, 2, true, false); assert_eq!(ecc.read(0x1c) & VERIFIED, 0);
        start(&mut ecc, 6, true, false); assert_eq!(ecc.read(0x1c) & VERIFIED, 0);
    }
    #[test]
    fn point_addition_scaled_jacobian_and_inverse_points() {
        let curve = Curve::new(true);
        let mut ecc = Ecc::default();
        write(&mut ecc, 0x120, P256_POINT_X); write(&mut ecc, 0x140, P256_POINT_Y);
        // Q = P represented with Z=2: (4X, 8Y, 2).
        write(&mut ecc, 0x160, &mul(P256_POINT_X, &[4], &curve.p));
        write(&mut ecc, 0x180, &mul(P256_POINT_Y, &[8], &curve.p)); write(&mut ecc, 0x1a0, &[2]);
        start(&mut ecc, 5, true, false);
        // Independent OpenSSL P-256 2G coordinates (also used by the C6 tests).
        assert_eq!(read(&mut ecc, 0x120, 8), [0x47669978, 0xa60b48fc, 0x77f21b35, 0xc08969e2, 0x04b51ac3, 0x8a523803, 0x8d034f7e, 0x7cf27b18]);
        assert_eq!(read(&mut ecc, 0x140, 8), [0x227873d1, 0x9e04b79d, 0x3ce98229, 0xba7dade6, 0x9f7430db, 0x293d9ac6, 0xdb8ed040, 0x07775510]);
        write(&mut ecc, 0x120, P256_POINT_X); write(&mut ecc, 0x140, P256_POINT_Y);
        ecc.store(24, P256_POINT_X); ecc.store(32, &sub(&[], P256_POINT_Y, &curve.p)); ecc.store(40, &[1]);
        start(&mut ecc, 5, true, false);
        assert!(read(&mut ecc, 0x120, 40).iter().all(|v| *v == 0));
    }
    #[test]
    fn zero_and_order_scalar_return_infinity_and_p192_output_is_zero_padded() {
        for p256 in [false, true] {
            let (x, y) = if p256 { (P256_POINT_X, P256_POINT_Y) } else { (P192_POINT_X, P192_POINT_Y) };
            for key in [vec![], Curve::new(p256).order] {
                let mut ecc = Ecc::default();
                write(&mut ecc, 0x120, x); write(&mut ecc, 0x140, y);
                ecc.write(0x138, u32::MAX); ecc.write(0x158, u32::MAX);
                if p256 { write(&mut ecc, 0x120, x); write(&mut ecc, 0x140, y); }
                write(&mut ecc, 0x100, &key); start(&mut ecc, 0, p256, false);
                assert!(read(&mut ecc, 0x120, 16).iter().all(|v| *v == 0));
            }
        }
    }
    #[test]
    fn c61_register_layout_masks_irq_reset_and_reserved_accesses() {
        let mut ecc = Ecc::default();
        assert_eq!(ecc.read(0x1c), 0); assert_eq!(ecc.read(0xfc), 37752928);
        write(&mut ecc, 0x120, P256_POINT_X); write(&mut ecc, 0x140, P256_POINT_Y);
        start(&mut ecc, 2, true, false);
        assert_eq!(ecc.irq_sources(), 0); ecc.write(0x14, u32::MAX); assert_eq!(ecc.irq_sources(), 1);
        assert_eq!(ecc.read(0x10), 1); assert_eq!(ecc.read(0x14), 1);
        ecc.write(0x18, 0); assert_eq!(ecc.read(0xc), 1); ecc.write(0x18, 1); assert_eq!(ecc.irq_sources(), 0);
        // Software cannot forge verification; bit8 is C61 SECURITY_MODE.
        ecc.write(0x1c, 1 << 8); assert_eq!(ecc.read(0x1c), VERIFIED | 256);
        ecc.write(0x1c, 2); assert_eq!(ecc.read(0x1c), 0); assert_eq!(ecc.read(0x120), 0);
        ecc.write(0x1c, VERIFIED | 256); assert_eq!(ecc.read(0x1c), 256);
        ecc.write(0x1bc, u32::MAX); assert_eq!(ecc.read(0x1bc), u32::MAX);
        for off in [0x100 - 1, 0x101, 0x1bd, 0x1c0, u32::MAX] { ecc.write(off, u32::MAX); assert_eq!(ecc.read(off), 0); }
        ecc.write(0x1c, (1 << 4) | 1); assert_eq!(ecc.read(0xc), 0);
        ecc.write(0x1c, (15 << 4) | 1); assert_eq!(ecc.read(0xc), 0);
        ecc.reset(); assert_eq!(ecc.read(0x1bc), 0);
        ecc.write(0xfc, u32::MAX); assert_eq!(ecc.read(0xfc), 0x0fffffff);
        start(&mut ecc, 2, true, false); ecc.write(0xc, 1); assert_eq!(ecc.read(0xc), 0);
    }
}
