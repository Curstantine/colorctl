use anyhow::{Context, Result, bail, format_err};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::io;

pub mod cli;
pub mod fan;
pub mod rgb;
pub mod utils;

use cli::{Cli, Commands, FanAction, FanArgs, RgbAction, RgbArgs, RgbTarget};
use fan::{FanHeader, SuperIo};
use rgb::{Channel, RgbController, parse_color};
use utils::pct;

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
                println!("  State File:  {:?}", ctrl.state_path());
                println!("  Channels:");
                for ch in Channel::all() {
                    let range = ch.led_range();
                    let count = range.end - range.start;
                    let cfg = ctrl.get_channel_config(*ch);
                    if cfg.enabled {
                        println!(
                            "    - {:<28} ({:>2} LEDs) : #{:02x}{:02x}{:02x} ({}%)",
                            ch.name(),
                            count,
                            cfg.color[0],
                            cfg.color[1],
                            cfg.color[2],
                            cfg.brightness
                        );
                    } else {
                        println!("    - {:<28} ({:>2} LEDs) : off", ch.name(), count);
                    }
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

            let is_all = channel.contains(&RgbTarget::All);
            let targets = if is_all {
                Channel::all().to_vec()
            } else {
                let mut list = Vec::new();
                for t in &channel {
                    for ch in t.channels() {
                        if !list.contains(ch) {
                            list.push(*ch);
                        }
                    }
                }
                list
            };

            for ch in &targets {
                ctrl.set_channel_color(*ch, rgb_val, brightness);
            }
            ctrl.apply()?;

            if is_all {
                println!(
                    "Set all channels to #{:02x}{:02x}{:02x} at {}% brightness",
                    rgb_val[0], rgb_val[1], rgb_val[2], brightness
                );
            } else {
                for ch in &targets {
                    println!(
                        "Set channel '{}' to #{:02x}{:02x}{:02x} at {}% brightness",
                        ch.name(),
                        rgb_val[0],
                        rgb_val[1],
                        rgb_val[2],
                        brightness
                    );
                }
            }
        }
        RgbAction::Off { channel } => {
            let mut ctrl = RgbController::find().context(
                "Failed to connect to RGB controller. Is the RGB controller enabled in BIOS?",
            )?;

            let is_all = channel.contains(&RgbTarget::All);
            let targets = if is_all {
                Channel::all().to_vec()
            } else {
                let mut list = Vec::new();
                for t in &channel {
                    for ch in t.channels() {
                        if !list.contains(ch) {
                            list.push(*ch);
                        }
                    }
                }
                list
            };

            for ch in &targets {
                ctrl.turn_off_channel(*ch);
            }
            ctrl.apply()?;

            if is_all {
                println!("Turned off all RGB channels");
            } else {
                for ch in &targets {
                    println!("Turned off channel '{}'", ch.name());
                }
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
            println!("  Motherboard Temp: {} °C", sio.get_temperature()?);
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
                (Some(p), None) => pct(p),
                (None, Some(pwm)) => pwm,
                (Some(_), Some(_)) => bail!("Specify either --percent or --pwm, not both"),
                (None, None) => bail!("Must specify either --percent (0-100) or --pwm (0-255)"),
            };

            for h in fan.headers() {
                sio.set_fan_speed_pwm(*h, pwm_val)?;
                println!(
                    "Set {} to PWM {} (~{}%)",
                    h.name(),
                    pwm_val,
                    ((pwm_val as f64 / 255.0) * 100.0).round()
                );
            }
        }
        FanAction::SetCurve {
            fan,
            points,
            profile,
        } => {
            let headers = fan.headers();

            match (points, profile) {
                (Some(parsed_points), None) => {
                    for h in headers {
                        sio.set_fan_curve(*h, &parsed_points)?;
                        println!("Configured 4-point SmartFan curve on {}:", h.name());
                        for (i, (t, p)) in parsed_points.iter().enumerate() {
                            let pct = ((*p as f64 / 255.0) * 100.0).round();
                            println!("    Point {}: {} °C -> PWM {} (~{}%)", i + 1, t, p, pct);
                        }
                    }
                }
                (None, Some(prof)) => {
                    for h in headers {
                        let curve = prof.points_for(*h == FanHeader::Pump);
                        sio.set_fan_curve(*h, &curve)?;
                        println!("Applied '{:?}' curve profile on {}:", prof, h.name());
                        for (i, (t, p)) in curve.iter().enumerate() {
                            let pct = ((*p as f64 / 255.0) * 100.0).round();
                            println!("    Point {}: {} °C -> PWM {} (~{}%)", i + 1, t, p, pct);
                        }
                    }
                }
                (Some(_), Some(_)) => {
                    bail!("Cannot specify both --points and --profile; choose one");
                }
                (None, None) => {
                    bail!(
                        "Must specify either --points 'T1:P1,T2:P2,T3:P3,T4:P4' or --profile <quiet|standard|full>"
                    );
                }
            }
        }
    }

    Ok(())
}
