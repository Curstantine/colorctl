# colorctl

Hardware control utility written in Rust for the **Colorful CVN B650M Gaming Frozen V14** motherboard, providing RGB lighting and fan curve management.

## Features

- **RGB Lighting** (No root required): Control onboard LEDs, 12V 4-pin RGB headers, and 5V 3-pin ARGB headers via `/dev/hidraw`.
- **Fan & Pump Control** (Requires root): Read real-time RPMs, monitor motherboard temperature, set manual PWM speeds, configure 4-point SmartFan curves, or apply built-in fan profiles (`quiet`, `standard`, `full`) via the Nuvoton NCT5584D Super I/O.
- **Shell Completions**: Native completion generation for Bash, Zsh, Fish, PowerShell, and Elvish.

## Usage

### RGB Control (No root required)

```bash
# Check RGB controller status and available channels
colorctl rgb status

# Set color across all channels
colorctl rgb set --color cyan
colorctl rgb set --color "#ff0088" --brightness 80

# Control specific channels
colorctl rgb set --channel led --color blue
colorctl rgb set --channel 5v-1 --color magenta

# Turn off lighting
colorctl rgb off
colorctl rgb off --channel led
```

**Channels**: `all`, `led` (onboard heatsink/IO), `12v-1`, `12v-2`, `5v-1`, `5v-2`, `5v-3`.

### Fan Control (Requires root)

> [!NOTE]
> Direct Super I/O port access requires root privileges (`doas`, `sudo`, or `su`).

```bash
# View fan RPMs and motherboard temperature
doas colorctl fan status

# Set fixed manual fan speed (percentage or raw PWM 0-255)
doas colorctl fan set-speed --fan cpu --percent 60
doas colorctl fan set-speed --fan all --percent 50
doas colorctl fan set-speed --fan sys1 --pwm 180

# Apply built-in SmartFan curve profiles
doas colorctl fan set-curve --fan cpu --profile quiet
doas colorctl fan set-curve --fan all --profile standard
doas colorctl fan set-curve --fan all --profile full

# Configure a custom 4-point SmartFan curve (Temp_C:Speed_Percent)
doas colorctl fan set-curve --fan cpu --points 30:25,50:45,70:75,85:100
doas colorctl fan set-curve --fan all --points 30:20,50:40,70:70,85:100
```

**Fan headers**: `all`, `cpu`, `sys1` (`CHA_FAN1`), `sys2` (`CHA_FAN2`), `sys3` (`CHA_FAN3`), `pump` (`AIO_PUMP`).

**Built-in Curve Profiles** (directly from Colorful iGame specifications):

- **`quiet`**: Minimal acoustic noise (CPU/Chassis: `20°C:20%, 40°C:30%, 60°C:50%, 80°C:85%`; Pump: `0°C:80%, 70°C:80%, 75°C:100%, 100°C:100%`)
- **`standard`**: Balanced daily profile (CPU/Chassis: `20°C:20%, 40°C:40%, 60°C:60%, 80°C:100%`; Pump: `0°C:80%, 65°C:80%, 70°C:100%, 100°C:100%`)
- **`full`**: Maximum cooling performance (100% across all temperatures)

### Shell Completions

Generate completions for your shell of choice:

```bash
# Fish
colorctl completions fish > ~/.config/fish/completions/colorctl.fish

# Zsh
colorctl completions zsh > ~/.zfunc/_colorctl

# Bash
colorctl completions bash > ~/.local/share/bash-completion/completions/colorctl
```

---

## Technical Reference

- [iGame Center Lite](https://www.colorfulgroup.com/en/igamecenter): implementation reference.
- **RGB Controller**: USB HID device (`VID: 0x2F4C`, `PID: 0x1000`, Usage Page `0xFF01`), up to 200 LEDs across 10 report packets + commit packet.
- **Super I/O**: Nuvoton NCT5584D (`Chip ID: 0xD42A`), accessed via config port `0x4E` and HWM base port `0x0A20` via `/dev/port`.
