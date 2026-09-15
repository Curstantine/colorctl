#[inline]
pub const fn pct(p: u8) -> u8 {
    let p = if p > 100 { 100 } else { p };
    ((p * 255 + 99) / 100) as u8
}
