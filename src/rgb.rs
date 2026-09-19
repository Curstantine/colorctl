use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::utils;

pub const COLORFUL_VID: u16 = 0x2f4c;
pub const COLORFUL_PID: u16 = 0x1000;
pub const TOTAL_LEDS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Led,   // Onboard Motherboard RGB (18 LEDs, index 0..17)
    Rgb1,  // 12V_1 (1 LED, index 18)
    Rgb2,  // 12V_2 (1 LED, index 19)
    Argb1, // 5V_1 (60 LEDs, index 20..79)
    Argb2, // 5V_2 (60 LEDs, index 80..139)
    Argb3, // 5V_3 (60 LEDs, index 140..199)
}

impl Channel {
    pub fn name(&self) -> &'static str {
        match self {
            Channel::Led => "LED (Onboard)",
            Channel::Rgb1 => "12V_1 (12V RGB Header 1)",
            Channel::Rgb2 => "12V_2 (12V RGB Header 2)",
            Channel::Argb1 => "5V_1 (ARGB Header 1)",
            Channel::Argb2 => "5V_2 (ARGB Header 2)",
            Channel::Argb3 => "5V_3 (ARGB Header 3)",
        }
    }

    pub fn led_range(&self) -> std::ops::Range<usize> {
        match self {
            Channel::Led => 0..18,
            Channel::Rgb1 => 18..19,
            Channel::Rgb2 => 19..20,
            Channel::Argb1 => 20..80,
            Channel::Argb2 => 80..140,
            Channel::Argb3 => 140..200,
        }
    }

    #[inline]
    pub fn all() -> &'static [Channel] {
        &[
            Channel::Led,
            Channel::Rgb1,
            Channel::Rgb2,
            Channel::Argb1,
            Channel::Argb2,
            Channel::Argb3,
        ]
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelConfig {
    pub color: [u8; 3],
    pub brightness: u8,
    pub enabled: bool,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            color: [0, 0, 0],
            brightness: 0,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbState {
    pub channels: HashMap<Channel, ChannelConfig>,
}

impl Default for RgbState {
    fn default() -> Self {
        let mut channels = HashMap::new();
        for ch in Channel::all() {
            channels.insert(*ch, ChannelConfig::default());
        }
        Self { channels }
    }
}

impl RgbState {
    pub const MAGIC: &'static [u8; 4] = b"CCTL";
    pub const VERSION: u8 = 1;

    /// Serializes state into a compact 35-byte binary format:
    /// [0..4]: b"CCTL" magic bytes
    /// [4]: version (1)
    /// [5..35]: 6 channels * 5 bytes (R, G, B, Brightness, Enabled)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(5 + Channel::all().len() * 5);
        bytes.extend_from_slice(Self::MAGIC);
        bytes.push(Self::VERSION);

        for ch in Channel::all() {
            let cfg = self.channels.get(ch).cloned().unwrap_or_default();
            bytes.push(cfg.color[0]);
            bytes.push(cfg.color[1]);
            bytes.push(cfg.color[2]);
            bytes.push(cfg.brightness);
            bytes.push(if cfg.enabled { 1 } else { 0 });
        }

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 5 + Channel::all().len() * 5 {
            return None;
        }
        if &bytes[0..4] != Self::MAGIC || bytes[4] != Self::VERSION {
            return None;
        }

        let mut channels = HashMap::new();
        let mut offset = 5;
        for ch in Channel::all() {
            let r = bytes[offset];
            let g = bytes[offset + 1];
            let b = bytes[offset + 2];
            let brightness = bytes[offset + 3];
            let enabled = bytes[offset + 4] != 0;
            offset += 5;

            channels.insert(
                *ch,
                ChannelConfig {
                    color: [r, g, b],
                    brightness,
                    enabled,
                },
            );
        }

        Some(Self { channels })
    }

    pub fn to_leds(&self) -> Vec<[u8; 3]> {
        let mut leds = vec![[0, 0, 0]; TOTAL_LEDS];
        for ch in Channel::all() {
            if let Some(cfg) = self.channels.get(ch) {
                if cfg.enabled {
                    let scaled = RgbController::scale_color(cfg.color, cfg.brightness);
                    for i in ch.led_range() {
                        leds[i] = scaled;
                    }
                }
            }
        }
        leds
    }
}

pub struct RgbController {
    dev_path: PathBuf,
    state_path: PathBuf,
    state: RgbState,
    leds: Vec<[u8; 3]>,
}

impl RgbController {
    pub fn find() -> Result<Self> {
        let dev_path = Self::find_device_path()?;
        let (state_path, state) = Self::load_state();
        let leds = state.to_leds();
        Ok(Self {
            dev_path,
            state_path,
            state,
            leds,
        })
    }

    pub fn device_path(&self) -> &Path {
        &self.dev_path
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    pub fn get_channel_config(&self, channel: Channel) -> ChannelConfig {
        self.state
            .channels
            .get(&channel)
            .cloned()
            .unwrap_or_default()
    }

    pub fn resolve_state_path() -> PathBuf {
        utils::resolve_state_path("rgb.state")
    }

    pub fn load_state() -> (PathBuf, RgbState) {
        let path = Self::resolve_state_path();
        if path.is_file() {
            if let Ok(bytes) = fs::read(&path) {
                if let Some(state) = RgbState::from_bytes(&bytes) {
                    return (path, state);
                }
            }
        }

        (path, RgbState::default())
    }

    pub fn save_state(&self) -> Result<()> {
        utils::ensure_dir_permissions(&self.state_path).with_context(|| {
            format!("Failed to create state directory for {:?}", self.state_path)
        })?;

        fs::write(&self.state_path, self.state.to_bytes())
            .with_context(|| format!("Failed to write RGB state to {:?}", self.state_path))?;

        utils::set_world_writable(&self.state_path).with_context(|| {
            format!(
                "Failed to set world-writable permissions for {:?}",
                self.state_path
            )
        })?;

        Ok(())
    }

    pub fn find_device_path() -> Result<PathBuf> {
        let hidraw_dir = Path::new("/sys/class/hidraw");
        if !hidraw_dir.exists() {
            bail!("/sys/class/hidraw does not exist");
        }

        for entry in fs::read_dir(hidraw_dir)? {
            let entry = entry?;
            let device_path = entry.path().join("device");
            let uevent_path = device_path.join("uevent");
            let report_desc_path = device_path.join("report_descriptor");

            if let Ok(uevent) = fs::read_to_string(&uevent_path) {
                let mut is_target_id = false;
                for line in uevent.lines() {
                    if line.starts_with("HID_ID=") {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() >= 3 {
                            // HID_ID format: "BusType:VVVVVVVV:PPPPPPPP" (8-char hex, zero-padded)
                            let vid = u32::from_str_radix(parts[1].trim(), 16)
                                .unwrap_or(0) as u16;
                            let pid = u32::from_str_radix(parts[2].trim(), 16)
                                .unwrap_or(0) as u16;
                            if vid == COLORFUL_VID && pid == COLORFUL_PID {
                                is_target_id = true;
                            }
                        }
                    }
                }

                if is_target_id {
                    if let Ok(desc) = fs::read(&report_desc_path) {
                        // Usage Page 0xFF01: 0x06, 0x01, 0xFF
                        if desc.windows(3).any(|w| w == [0x06, 0x01, 0xff]) {
                            let dev_name = entry.file_name();
                            let dev_path = Path::new("/dev").join(dev_name);
                            return Ok(dev_path);
                        }
                    }
                }
            }
        }

        bail!(
            "Colorful Motherboard RGB controller (VID: 0x{:04X}, PID: 0x{:04X}, UsagePage: 0xFF01) not found",
            COLORFUL_VID,
            COLORFUL_PID
        )
    }

    pub fn set_channel_color(&mut self, channel: Channel, color: [u8; 3], brightness: u8) {
        self.state.channels.insert(
            channel,
            ChannelConfig {
                color,
                brightness,
                enabled: true,
            },
        );
        let scaled_color = Self::scale_color(color, brightness);
        for i in channel.led_range() {
            self.leds[i] = scaled_color;
        }
    }

    pub fn turn_off_channel(&mut self, channel: Channel) {
        self.state.channels.insert(
            channel,
            ChannelConfig {
                color: [0, 0, 0],
                brightness: 0,
                enabled: false,
            },
        );
        for i in channel.led_range() {
            self.leds[i] = [0, 0, 0];
        }
    }

    pub fn scale_color(color: [u8; 3], brightness: u8) -> [u8; 3] {
        let factor = brightness.min(100) as u16;
        [
            ((color[0] as u16 * factor) / 100) as u8,
            ((color[1] as u16 * factor) / 100) as u8,
            ((color[2] as u16 * factor) / 100) as u8,
        ]
    }

    pub fn apply(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.dev_path)
            .with_context(|| format!("Failed to open HID device {:?}", self.dev_path))?;

        // Some HID drivers expect a leading Report ID byte (65-byte write); others
        // don't (64-byte write). We detect which on the first packet and reuse that
        // mode for the remaining packets and the flush packet.
        let use_report_id = Self::write_hid_packet(&mut file, true)?;

        // 10 packets of 20 LEDs each
        for (pkt_idx, chunk) in self.leds.chunks(20).enumerate() {
            if pkt_idx == 0 {
                // Already sent packet 0 during probing above; skip.
                continue;
            }
            let mut pkt = [0u8; 65];
            pkt[0] = 0x00; // Report ID
            pkt[1] = 0x01;
            pkt[2] = 0x00;
            pkt[3] = 0x88;
            pkt[4] = pkt_idx as u8;

            for (i, rgb) in chunk.iter().enumerate() {
                pkt[5 + i * 3] = rgb[0];
                pkt[5 + i * 3 + 1] = rgb[1];
                pkt[5 + i * 3 + 2] = rgb[2];
            }

            if use_report_id {
                file.write_all(&pkt)?;
            } else {
                file.write_all(&pkt[1..])?;
            }
        }

        // Flush / Commit packet
        let mut flush_pkt = [0u8; 65];
        flush_pkt[0] = 0x00;
        flush_pkt[1] = 0x01;
        flush_pkt[2] = 0x00;
        flush_pkt[3] = 0x88;
        flush_pkt[4] = 0xff;

        if use_report_id {
            file.write_all(&flush_pkt)?;
        } else {
            file.write_all(&flush_pkt[1..])?;
        }

        self.save_state()?;

        Ok(())
    }

    /// Sends LED packet 0 and detects whether the driver expects a Report ID prefix.
    ///
    /// Returns `true` if 65-byte writes (with Report ID) are expected,
    /// or `false` if 64-byte writes (without Report ID) are expected.
    /// Only retries on `EINVAL`, which is what the kernel returns when it
    /// doesn't want a leading Report ID byte.
    fn write_hid_packet(file: &mut std::fs::File, _first: bool) -> Result<bool> {
        let pkt = [0u8; 65]; // Packet 0 with no LEDs set yet (will be overwritten in the loop)
        match file.write_all(&pkt) {
            Ok(()) => Ok(true),
            Err(e) if e.raw_os_error() == Some(libc::EINVAL) => {
                // Kernel rejected the 65-byte write — retry without Report ID prefix
                file.write_all(&pkt[1..])?;
                Ok(false)
            }
            Err(e) => Err(e).with_context(|| "Failed to write to HID device"),
        }
    }
}

pub fn parse_color(s: &str) -> Result<[u8; 3]> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "red" => return Ok([255, 0, 0]),
        "green" => return Ok([0, 255, 0]),
        "blue" => return Ok([0, 0, 255]),
        "white" => return Ok([255, 255, 255]),
        "black" | "off" => return Ok([0, 0, 0]),
        "yellow" => return Ok([255, 255, 0]),
        "cyan" => return Ok([0, 255, 255]),
        "magenta" | "purple" => return Ok([255, 0, 255]),
        "orange" => return Ok([255, 128, 0]),
        _ => {}
    }

    let hex_str = s.trim_start_matches('#').trim_start_matches("0x");
    if hex_str.len() == 6 {
        let r = u8::from_str_radix(&hex_str[0..2], 16).context("Invalid hex red")?;
        let g = u8::from_str_radix(&hex_str[2..4], 16).context("Invalid hex green")?;
        let b = u8::from_str_radix(&hex_str[4..6], 16).context("Invalid hex blue")?;
        Ok([r, g, b])
    } else {
        bail!(
            "Invalid color format '{s}'. Expected hex (e.g. 'ff00aa', '#00ff00') or name ('red', 'blue', 'green', 'white', 'off')"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_state_channel_preservation() {
        let mut state = RgbState::default();
        for ch in Channel::all() {
            assert!(!state.channels.get(ch).unwrap().enabled);
        }

        // Set 5V_1 to red
        state.channels.insert(
            Channel::Argb1,
            ChannelConfig {
                color: [255, 0, 0],
                brightness: 100,
                enabled: true,
            },
        );

        // Set 5V_2 to blue
        state.channels.insert(
            Channel::Argb2,
            ChannelConfig {
                color: [0, 0, 255],
                brightness: 80,
                enabled: true,
            },
        );

        // 5V_1 must still be red and enabled!
        let argb1 = state.channels.get(&Channel::Argb1).unwrap();
        assert!(argb1.enabled);
        assert_eq!(argb1.color, [255, 0, 0]);

        // 5V_2 must be blue and enabled!
        let argb2 = state.channels.get(&Channel::Argb2).unwrap();
        assert!(argb2.enabled);
        assert_eq!(argb2.color, [0, 0, 255]);

        // Check the generated LED buffer
        let leds = state.to_leds();
        for i in Channel::Argb1.led_range() {
            assert_eq!(leds[i], [255, 0, 0]);
        }
        let expected_blue = RgbController::scale_color([0, 0, 255], 80);
        for i in Channel::Argb2.led_range() {
            assert_eq!(leds[i], expected_blue);
        }
        for i in Channel::Led.led_range() {
            assert_eq!(leds[i], [0, 0, 0]);
        }
    }

    #[test]
    fn test_rgb_state_binary_serialization() {
        let mut state = RgbState::default();
        state.channels.insert(
            Channel::Argb1,
            ChannelConfig {
                color: [255, 0, 0],
                brightness: 100,
                enabled: true,
            },
        );
        let bytes = state.to_bytes();
        assert_eq!(bytes.len(), 35);
        assert_eq!(&bytes[0..4], b"CCTL");
        assert_eq!(bytes[4], 1);

        let deserialized = RgbState::from_bytes(&bytes).unwrap();
        let cfg = deserialized.channels.get(&Channel::Argb1).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.color, [255, 0, 0]);
        assert_eq!(cfg.brightness, 100);

        // Invalid magic / truncated bytes return None
        assert!(RgbState::from_bytes(&[0, 1, 2]).is_none());
        assert!(RgbState::from_bytes(&[b'X', b'X', b'X', b'X', 1]).is_none());
    }
}
