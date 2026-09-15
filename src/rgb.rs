use anyhow::{Context, Result, bail};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const COLORFUL_VID: u16 = 0x2f4c;
pub const COLORFUL_PID: u16 = 0x1000;
pub const TOTAL_LEDS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn from_str(s: &str) -> Option<Channel> {
        match s.to_ascii_lowercase().as_str() {
            "led" | "onboard" | "0" => Some(Channel::Led),
            "12v_1" | "12v-1" | "rgb1" | "1" => Some(Channel::Rgb1),
            "12v_2" | "12v-2" | "rgb2" | "2" => Some(Channel::Rgb2),
            "5v_1" | "5v-1" | "argb1" | "3" => Some(Channel::Argb1),
            "5v_2" | "5v-2" | "argb2" | "4" => Some(Channel::Argb2),
            "5v_3" | "5v-3" | "argb3" | "5" => Some(Channel::Argb3),
            _ => None,
        }
    }
}

pub struct RgbController {
    dev_path: PathBuf,
    leds: Vec<[u8; 3]>,
}

impl RgbController {
    pub fn find() -> Result<Self> {
        let dev_path = Self::find_device_path()?;
        Ok(Self {
            dev_path,
            leds: vec![[0, 0, 0]; TOTAL_LEDS],
        })
    }

    pub fn device_path(&self) -> &Path {
        &self.dev_path
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
                            let vid = u16::from_str_radix(parts[1].trim_start_matches("0000"), 16)
                                .unwrap_or(0);
                            let pid = u16::from_str_radix(parts[2].trim_start_matches("0000"), 16)
                                .unwrap_or(0);
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
        let scaled_color = Self::scale_color(color, brightness);
        for i in channel.led_range() {
            self.leds[i] = scaled_color;
        }
    }

    pub fn set_all_color(&mut self, color: [u8; 3], brightness: u8) {
        let scaled_color = Self::scale_color(color, brightness);
        for led in &mut self.leds {
            *led = scaled_color;
        }
    }

    fn scale_color(color: [u8; 3], brightness: u8) -> [u8; 3] {
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

        // 10 packets of 20 LEDs each
        for (pkt_idx, chunk) in self.leds.chunks(20).enumerate() {
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

            if let Err(_) = file.write_all(&pkt) {
                // Try 64 bytes without leading Report ID
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

        if let Err(_) = file.write_all(&flush_pkt) {
            file.write_all(&flush_pkt[1..])?;
        }

        Ok(())
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
