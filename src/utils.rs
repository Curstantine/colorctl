#[inline]
/// Converts a percentage (0-100) to a PWM value (0-255)
pub const fn pct(p: u8) -> u8 {
    let p = if p > 100 { 100 } else { p };
    ((p as u32 * 255 + 99) / 100) as u8
}
