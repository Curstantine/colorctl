use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::fan::FanHeader;
use crate::rgb::Channel;
use crate::utils::pct;

#[derive(Parser, Debug)]
#[command(name = "colorctl")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Motherboard RGB Lighting Control (USB HID: 2F4C:1000)
    Rgb(RgbArgs),
    /// Motherboard Fan & Hardware Monitor Control (NCT5584D Super I/O, requires root)
    Fan(FanArgs),
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RgbTarget {
    All,
    #[value(name = "led", alias = "onboard")]
    Led,
    #[value(name = "12V_1", alias = "12v_1", alias = "rgb1")]
    Rgb1,
    #[value(name = "12V_2", alias = "12v_2", alias = "rgb2")]
    Rgb2,
    #[value(name = "5V_1", alias = "5v_1", alias = "argb1")]
    Argb1,
    #[value(name = "5V_2", alias = "5v_2", alias = "argb2")]
    Argb2,
    #[value(name = "5V_3", alias = "5v_3", alias = "argb3")]
    Argb3,
}

impl RgbTarget {
    pub fn channels(&self) -> &'static [Channel] {
        match self {
            RgbTarget::All => Channel::all(),
            RgbTarget::Led => &[Channel::Led],
            RgbTarget::Rgb1 => &[Channel::Rgb1],
            RgbTarget::Rgb2 => &[Channel::Rgb2],
            RgbTarget::Argb1 => &[Channel::Argb1],
            RgbTarget::Argb2 => &[Channel::Argb2],
            RgbTarget::Argb3 => &[Channel::Argb3],
        }
    }
}

#[derive(Args, Debug)]
pub struct RgbArgs {
    #[command(subcommand)]
    pub action: RgbAction,
}

#[derive(Subcommand, Debug)]
pub enum RgbAction {
    /// Detect and show RGB controller status
    Status,
    /// Set RGB color for a channel or all channels
    Set {
        /// Color to set: hex (e.g. 'ff0000', '#00ff88') or name ('red', 'green', 'blue', 'white', 'purple', etc.)
        #[arg(short = 'C', long)]
        color: String,

        /// Target RGB channel(s)
        #[arg(short = 'c', long, value_enum, value_delimiter = ',', default_value = "all")]
        channel: Vec<RgbTarget>,

        /// Brightness percentage (0-100)
        #[arg(short = 'b', long, default_value = "100")]
        brightness: u8,
    },
    /// Turn off RGB lighting
    Off {
        /// Target RGB channel(s)
        #[arg(short = 'c', long, value_enum, value_delimiter = ',', default_value = "all")]
        channel: Vec<RgbTarget>,
    },
}

#[derive(Args, Debug)]
pub struct FanArgs {
    #[command(subcommand)]
    pub action: FanAction,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanTarget {
    All,
    #[value(name = "cpu", alias = "cpu_fan")]
    Cpu,
    #[value(name = "cha_fan1")]
    ChaFan1,
    #[value(name = "cha_fan2")]
    ChaFan2,
    #[value(name = "cha_fan3")]
    ChaFan3,
    #[value(name = "pump", alias = "aio_pump")]
    Pump,
}

impl FanTarget {
    pub fn headers(&self) -> &'static [FanHeader] {
        match self {
            FanTarget::All => FanHeader::all(),
            FanTarget::Cpu => &[FanHeader::Cpu],
            FanTarget::ChaFan1 => &[FanHeader::ChaFan1],
            FanTarget::ChaFan2 => &[FanHeader::ChaFan2],
            FanTarget::ChaFan3 => &[FanHeader::ChaFan3],
            FanTarget::Pump => &[FanHeader::Pump],
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanProfile {
    /// Quiet curve for low noise (20°C:20%, 40°C:30%, 60°C:50%, 80°C:85%)
    Quiet,
    /// Balanced standard curve (20°C:20%, 40°C:40%, 60°C:60%, 80°C:100%)
    Standard,
    /// Maximum cooling performance (100% across all temperatures)
    Full,
}

impl FanProfile {
    const PROFILE_QUIET: [(u8, u8); 4] =
        [(20, pct(20)), (40, pct(30)), (60, pct(50)), (80, pct(85))];
    const PROFILE_STANDARD: [(u8, u8); 4] =
        [(20, pct(20)), (40, pct(40)), (60, pct(60)), (80, pct(100))];
    const PROFILE_PUMP_QUIET: [(u8, u8); 4] =
        [(0, pct(80)), (70, pct(80)), (75, pct(100)), (100, pct(100))];
    const PROFILE_PUMP_STANDARD: [(u8, u8); 4] =
        [(0, pct(80)), (65, pct(80)), (70, pct(100)), (100, pct(100))];
    const PROFILE_FULL: [(u8, u8); 4] = [
        (30, pct(100)),
        (50, pct(100)),
        (70, pct(100)),
        (100, pct(100)),
    ];

    pub fn points_for(&self, is_pump: bool) -> [(u8, u8); 4] {
        match self {
            FanProfile::Quiet if is_pump => Self::PROFILE_PUMP_QUIET,
            FanProfile::Quiet => Self::PROFILE_QUIET,
            FanProfile::Standard if is_pump => Self::PROFILE_PUMP_STANDARD,
            FanProfile::Standard => Self::PROFILE_STANDARD,
            FanProfile::Full => Self::PROFILE_FULL,
        }
    }
}

pub fn parse_curve_points(s: &str) -> Result<[(u8, u8); 4], String> {
    let mut points = [(0u8, 0u8); 4];
    let mut parts = s.split(',').map(str::trim);

    for (i, slot) in points.iter_mut().enumerate() {
        let part = parts.next().ok_or_else(|| {
            format!("expected exactly 4 points in format 'T1:P1,...', got fewer ({i})")
        })?;

        let mut pair = part.split(':').map(str::trim);
        let (temp_str, speed_str) = match (pair.next(), pair.next(), pair.next()) {
            (Some(t), Some(p), None) => (t, p),
            _ => {
                return Err(format!(
                    "invalid point '{part}': expected 'Temp:SpeedPercent' (e.g. '50:40')"
                ));
            }
        };

        let temp_c = temp_str
            .parse::<u8>()
            .map_err(|e| format!("invalid temperature '{temp_str}': {e}"))?;
        let percent = speed_str
            .parse::<u8>()
            .map_err(|e| format!("invalid speed percent '{speed_str}': {e}"))?;

        *slot = (temp_c, pct(percent));
    }

    if parts.next().is_some() {
        return Err("expected exactly 4 points, got more than 4".to_string());
    }

    Ok(points)
}

#[derive(Subcommand, Debug)]
pub enum FanAction {
    /// Show current fan RPM speeds, temperatures, and Super I/O status
    Status,
    /// Set manual fan speed (0-100% or raw PWM 0-255)
    SetSpeed {
        /// Target fan header
        #[arg(short = 'f', long, value_enum, default_value = "all")]
        fan: FanTarget,

        /// Speed percentage (0-100)
        #[arg(short = 'p', long, value_parser = clap::value_parser!(u8).range(0..101))]
        percent: Option<u8>,

        /// Raw PWM duty cycle (0-255)
        #[arg(long)]
        pwm: Option<u8>,
    },
    /// Configure 4-point SmartFan temperature curve or apply a built-in profile
    SetCurve {
        /// Target fan header
        #[arg(short = 'f', long, value_enum, default_value = "all")]
        fan: FanTarget,

        /// 4 temperature-speed points in format: 'T1:P1,T2:P2,T3:P3,T4:P4' (e.g. '30:20,50:40,70:70,85:100')
        #[arg(short = 'p', long, value_parser = parse_curve_points)]
        points: Option<[(u8, u8); 4]>,

        /// Built-in profile: 'quiet', 'standard', or 'full'
        #[arg(short = 'P', long, value_enum)]
        profile: Option<FanProfile>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
