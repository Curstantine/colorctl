use anyhow::{Context, Result, bail, format_err};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::io;

mod cli;
mod fan;
mod rgb;

use cli::{Cli, Commands, FanAction, FanArgs, RgbAction, RgbArgs};
use fan::{FanHeader, SuperIo};
use rgb::{Channel, RgbController, parse_color};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Rgb(rgb_args) => handle_rgb(rgb_args),
        Commands::Fan(fan_args) => handle_fan(fan_args),
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut io::stdout());
            Ok(())
        }
    }
}

fn handle_rgb(args: RgbArgs) -> Result<()> {
    match args.action {
        RgbAction::Status => match RgbController::find() {
            Ok(ctrl) => {
                println!("Colorful RGB Controller detected:");
                println!("  Device Path: {:?}", ctrl.device_path());
                println!("  Channels available:");
                for ch in Channel::all() {
                    let range = ch.led_range();
                    println!("    - {:<28} ({} LEDs)", ch.name(), range.end - range.start);
                }
            }
            Err(e) => {
                println!("RGB Controller not detected: {}", e);
                println!("Note: If disabled in BIOS, please enable the onboard RGB controller.");
            }
        },
        RgbAction::Set {
            color,
            channel,
            brightness,
        } => {
            let rgb_val = parse_color(&color)?;
            let mut ctrl = RgbController::find().context(
                "Failed to connect to RGB controller. Is the RGB controller enabled in BIOS?",
            )?;

            if channel.eq_ignore_ascii_case("all") {
                ctrl.set_all_color(rgb_val, brightness);
                ctrl.apply()?;
                println!(
                    "Set all channels to #{:02x}{:02x}{:02x} at {}% brightness",
                    rgb_val[0], rgb_val[1], rgb_val[2], brightness
                );
            } else if let Some(ch) = Channel::from_str(&channel) {
                ctrl.set_channel_color(ch, rgb_val, brightness);
                ctrl.apply()?;
                println!(
                    "Set channel '{}' to #{:02x}{:02x}{:02x} at {}% brightness",
                    ch.name(),
                    rgb_val[0],
                    rgb_val[1],
                    rgb_val[2],
                    brightness
                );
            } else {
                bail!(
                    "Unknown channel '{}'. Choose from: all, led, 12v-1, 12v-2, 5v-1, 5v-2, 5v-3",
                    channel
                );
            }
        }
        RgbAction::Off { channel } => {
            let mut ctrl = RgbController::find().context(
                "Failed to connect to RGB controller. Is the RGB controller enabled in BIOS?",
            )?;

            if channel.eq_ignore_ascii_case("all") {
                ctrl.set_all_color([0, 0, 0], 0);
                ctrl.apply()?;
                println!("Turned off all RGB channels");
            } else if let Some(ch) = Channel::from_str(&channel) {
                ctrl.set_channel_color(ch, [0, 0, 0], 0);
                ctrl.apply()?;
                println!("Turned off channel '{}'", ch.name());
            } else {
                bail!(
                    "Unknown channel '{}'. Choose from: all, led, 12v-1, 12v-2, 5v-1, 5v-2, 5v-3",
                    channel
                );
            }
        }
    }
    Ok(())
}

fn handle_fan(args: FanArgs) -> Result<()> {
    let mut sio =
        SuperIo::open().map_err(|e| format_err!("Failed to access Super I/O chip:\n\t{e}"))?;

    match args.action {
        FanAction::Status => {
            println!(
                "Nuvoton NCT5584D Super I/O (Chip ID: 0x{:04X})",
                sio.chip_id
            );
            let temp = sio.get_temperature()?;
            println!("  Motherboard Temp: {} °C", temp);
            println!("  Fan Speeds:");
            for header in FanHeader::all() {
                let rpm = sio.get_fan_rpm(*header)?;
                println!(
                    "    - {:<10} (Bank {:2}): {:5} RPM",
                    header.name(),
                    header.bank(),
                    rpm
                );
            }
        }
        FanAction::SetSpeed { fan, percent, pwm } => {
            let pwm_val = match (percent, pwm) {
                (Some(p), None) => {
                    let p = p.min(100);
                    ((p as f64) * 2.55).ceil() as u8
                }
                (None, Some(pwm)) => pwm,
                (Some(_), Some(_)) => bail!("Specify either --percent or --pwm, not both"),
                (None, None) => bail!("Must specify either --percent (0-100) or --pwm (0-255)"),
            };

            let headers: Vec<FanHeader> = if fan.eq_ignore_ascii_case("all") {
                FanHeader::all().to_vec()
            } else if let Some(h) = FanHeader::from_str(&fan) {
                vec![h]
            } else {
                bail!(
                    "Unknown fan '{}'. Choose from: all, cpu, sys1, sys2, sys3, pump",
                    fan
                );
            };

            for h in headers {
                sio.set_fan_speed_pwm(h, pwm_val)?;
                println!(
                    "Set {} to PWM {} (~{}%)",
                    h.name(),
                    pwm_val,
                    ((pwm_val as f64 / 255.0) * 100.0).round()
                );
            }
        }
        FanAction::SetCurve { fan, points } => {
            let parsed_points = parse_curve_points(&points)?;

            let headers: Vec<FanHeader> = if fan.eq_ignore_ascii_case("all") {
                FanHeader::all().to_vec()
            } else if let Some(h) = FanHeader::from_str(&fan) {
                vec![h]
            } else {
                bail!(
                    "Unknown fan '{}'. Choose from: all, cpu, sys1, sys2, sys3, pump",
                    fan
                );
            };

            for h in headers {
                sio.set_fan_curve(h, &parsed_points)?;
                println!("Configured 4-point SmartFan curve on {}:", h.name());
                for (i, (t, p)) in parsed_points.iter().enumerate() {
                    let pct = ((*p as f64 / 255.0) * 100.0).round();
                    println!("    Point {}: {} °C -> PWM {} (~{}%)", i + 1, t, p, pct);
                }
            }
        }
    }

    Ok(())
}

fn parse_curve_points(s: &str) -> Result<[(u8, u8); 4]> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() != 4 {
        bail!(
            "Expected exactly 4 points in format 'T1:P1,T2:P2,T3:P3,T4:P4', got {}",
            parts.len()
        );
    }

    let mut points = [(0u8, 0u8); 4];
    for (i, part) in parts.iter().enumerate() {
        let pair: Vec<&str> = part.split(':').map(|p| p.trim()).collect();
        if pair.len() != 2 {
            bail!(
                "Invalid point '{}'. Expected 'Temp:SpeedPercent' (e.g. '50:40')",
                part
            );
        }
        let temp_c: u8 = pair[0].parse().context("Invalid temperature")?;
        let percent: u8 = pair[1].parse().context("Invalid speed percent")?;
        let pwm = ((percent.min(100) as f64) * 2.55).ceil() as u8;
        points[i] = (temp_c, pwm);
    }

    Ok(points)
}
