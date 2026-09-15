# colorctl

Hardware control utility written in Rust for **Colorful AMD motherboards** (tested on the **CVN B650M Gaming Frozen V14**, with broad support across B650/B850/X870 series), providing RGB lighting and fan curve management.

## Features

- **RGB Lighting** (No root required): Control onboard LEDs, 12V 4-pin RGB headers, and 5V 3-pin ARGB headers via `/dev/hidraw`.
- **Fan & Pump Control** (Requires root): Read real-time RPMs, monitor motherboard temperature, set manual PWM speeds, configure 4-point SmartFan curves, or apply built-in fan profiles (`quiet`, `standard`, `full`) via the Nuvoton NCT5584D Super I/O.
- **Shell Completions**: Native completion generation for Bash, Zsh, Fish, PowerShell, and Elvish.
- **NixOS Module**: Declarative boot/resume service to persist fan curves and RGB settings across reboots, with automatic udev rules.

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
> Super I/O port access requires root privileges (`doas`, `sudo`, or `su`).

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

## Nix & NixOS

### Run directly with Flakes

```bash
nix run github:Curstantine/colorctl -- rgb status
```

### NixOS Module (Persistence across reboots & resume)

To have your preferred fan curve and RGB configuration automatically applied on system boot and resume from suspend:

1. Add `colorctl` to your flake inputs:

    ```nix
    inputs = {
      colorctl = {
        url = "github:Curstantine/colorctl";
        inputs.nixpkgs.follows = "nixpkgs";
      };
    };
    ```

2. Import the module, add the overlay, and configure the service:
    ```nix
    { inputs, ... }:
    {
      imports = [ inputs.colorctl.nixosModules.default ];

      nixpkgs.overlays = [
        inputs.colorctl.overlays.default
      ];

      services.colorctl = {
        enable = true;

        # Fan curve configuration
        fan = {
          enable = true;
          header = "all";
          profile = "quiet"; # "quiet", "standard", or "full"
          # Or custom points:
          # customPoints = "30:25,50:45,70:75,85:100";
        };

        # RGB lighting configuration
        rgb = {
          enable = true;
          channel = "all";
          color = "cyan"; # color name or hex "#00ff88"
          brightness = 100;
        };
      };
    }
    ```

The module automatically:

- Installs `colorctl` with shell completions.
- Installs udev rules granting unprivileged users access to `/dev/hidraw*` for RGB lighting.
- Creates a `systemd.services.colorctl` oneshot service that executes on boot and after wake from suspend (`post-resume.target`).

---

## Note on Compatibility

While this is primary tested on the CVN B650M Gaming Frozen V14, it is designed to work across other B650, B850, and X870 models from Colorful as they share the same underlying Super I/O and USB HID hardware.

Supported / Compatible models include:

- CVN B650 Gaming Frozen V14
- CVN B650M Gaming Frozen V14
- COLORFIRE B650M-MEOW WIFI Orange
- BATTLE-AX B650M-PLUS V14 / V15
- BATTLE-AX B650M-WHITE WIFI V14 / V15
- CVN B850M Gaming Frozen V14
- CVN X870 Gaming Frozen V14

## Technical Reference

- [iGame Center Lite](https://www.colorfulgroup.com/en/igamecenter): implementation reference.
- **RGB Controller**: USB HID device (`VID: 0x2F4C`, `PID: 0x1000`, Usage Page `0xFF01`), up to 200 LEDs across 10 report packets + commit packet.
- **Super I/O**: Nuvoton NCT5584D (`Chip ID: 0xD42A`), accessed via config port `0x4E` and HWM base port `0x0A20` via `/dev/port`.

## Extracting & Inspecting the Official Driver Files

The reverse-engineering was performed on the official **iGame Center Lite** installer package (`iGC.Lite-*-Installer-Prod.exe`). If you want to extract and inspect the binaries yourself:

### 1. Extract the Installer

The installer executable is packaged using **Inno Setup (6.1.0)**. On Linux, extract its contents using `innoextract`:

```bash
# Using innoextract directly or via Nix:
innoextract iGC.Lite-*.exe -d extracted/
# or with Nix:
nix run nixpkgs#innoextract -- iGC.Lite-*.exe -d extracted/
```

This extracts the application binaries into `extracted/app/`.

### 2. Decompile the .NET Assemblies

The core logic (Super I/O drivers, motherboard sensor tables, fan curves, and LED services) is implemented in .NET C# assemblies. Decompile them using [`ilspycmd`](https://github.com/icsharpcode/ILSpy) (or a GUI decompiler like AvaloniaILSpy / dnSpy):

```bash
mkdir -p decompiled

# Decompile all application DLLs to C# projects:
for dll in extracted/app/*.dll; do
    name=$(basename "$dll" .dll)
    nix run nixpkgs#ilspycmd -- -p -o "decompiled/$name" "$dll"
done
```

### 3. Key Files for Reverse Engineering

- **`iGameCenter.Hardware/iGameCenter.Hardware.ComputerInfo/MBFanSensorHelper.cs`**:
  Contains motherboard identification lists (`NCT5584D_Datas`) and silkscreen header-to-index mappings for all supported Colorful boards.
- **`iGameAPI.MBoard.WinRing0/iGameAPI.MBoard.WinRing0.SuperIOChip/NCT5584D_Service.cs`**:
  Hardware Monitor base address discovery, Logical Device Number (`0x0B`) selection, bank assignments, and RPM register addresses.
- **`iGameAPI.MBoard.WinRing0/iGameAPI.MBoard.WinRing0/Colorful_SuperIO.cs`**:
  Super I/O config entry (`0x87, 0x87`), exit (`0xAA`), and I/O lock disabling routines.
- **`iGameCenter.ConfigManager/iGameCenter.ConfigManager/MBFanConfing.cs`**:
  Default factory 4-point SmartFan curves for quiet (`Mute`), standard (`General`), and full load (`FullLoad`) modes.
