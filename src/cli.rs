use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

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

        /// Channel to set: 'all', 'led' (onboard), '12v-1', '12v-2', '5v-1', '5v-2', '5v-3'
        #[arg(short = 'c', long, default_value = "all")]
        channel: String,

        /// Brightness percentage (0-100)
        #[arg(short = 'b', long, default_value = "100")]
        brightness: u8,
    },
    /// Turn off RGB lighting
    Off {
        /// Channel to turn off: 'all', 'led', '12v-1', '12v-2', '5v-1', '5v-2', '5v-3'
        #[arg(short = 'c', long, default_value = "all")]
        channel: String,
    },
}

#[derive(Args, Debug)]
pub struct FanArgs {
    #[command(subcommand)]
    pub action: FanAction,
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
    pub fn points_for(&self, is_pump: bool) -> [(u8, u8); 4] {
        let pct = |p: u8| ((p.min(100) as f64) * 2.55).ceil() as u8;
        match self {
            FanProfile::Quiet if is_pump => {
                [(0, pct(80)), (70, pct(80)), (75, pct(100)), (100, pct(100))]
            }
            FanProfile::Quiet => [(20, pct(20)), (40, pct(30)), (60, pct(50)), (80, pct(85))],
            FanProfile::Standard if is_pump => {
                [(0, pct(80)), (65, pct(80)), (70, pct(100)), (100, pct(100))]
            }
            FanProfile::Standard => [(20, pct(20)), (40, pct(40)), (60, pct(60)), (80, pct(100))],
            FanProfile::Full => [
                (30, pct(100)),
                (50, pct(100)),
                (70, pct(100)),
                (100, pct(100)),
            ],
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum FanAction {
    /// Show current fan RPM speeds, temperatures, and Super I/O status
    Status,
    /// Set manual fan speed (0-100% or raw PWM 0-255)
    SetSpeed {
        /// Fan header: 'cpu', 'sys1', 'sys2', 'sys3', 'pump', or 'all'
        #[arg(short = 'f', long)]
        fan: String,

        /// Speed percentage (0-100)
        #[arg(short = 'p', long)]
        percent: Option<u8>,

        /// Raw PWM duty cycle (0-255)
        #[arg(long)]
        pwm: Option<u8>,
    },
    /// Configure 4-point SmartFan temperature curve or apply a built-in profile
    SetCurve {
        /// Fan header: 'cpu', 'sys1', 'sys2', 'sys3', 'pump', or 'all'
        #[arg(short = 'f', long)]
        fan: String,

        /// 4 temperature-speed points in format: 'T1:P1,T2:P2,T3:P3,T4:P4' (e.g. '30:20,50:40,70:70,85:100')
        #[arg(short = 'p', long)]
        points: Option<String>,

        /// Built-in profile: 'quiet', 'standard', or 'full'
        #[arg(long, value_enum)]
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
