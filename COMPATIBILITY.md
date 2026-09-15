# Hardware Compatibility Guide

This document details hardware compatibility for **colorctl**, derived directly from the project's low-level implementation and reverse engineering of the official Colorful software (`iGameCenter` / `iGC.Lite` decompiled libraries: `iGameAPI.MBoard.WinRing0`, `iGameCenter.Hardware`, and `iGameAPI.dll`).

---

## 1. Architectural Overview

`colorctl` interacts with two separate motherboard subsystems:

1. **Fan & Thermal Management**:
    - Controller: Direct Super I/O port I/O (`/dev/port`) targeting the **Nuvoton NCT5584D** chip (Chip ID: `0xD42A` / `54314`).
    - Base Ports: Probed at `0x4E/0x4F` and `0x2E/0x2F`.
    - Hardware Monitor (HWM): Configured via Logical Device Number `0x0B` (LDN 11).
    - Fan Banks: Controls banks 1, 2, 3, 10, and 11 via registers `0x02` (mode), `0x09` (PWM), and `0x27`–`0x2E` (SmartFan 4-point curves).

2. **RGB & Lighting Control**:
    - Controller: Onboard USB HID microcontroller.
    - USB Identity: **Vendor ID (VID) `0x2F4C`**, **Product ID (PID) `0x1000`**.
    - Payload: A fixed 200-LED byte sequence covering 6 discrete channels (1 onboard, 2× 12V RGB, 3× 5V ARGB).

---

## 2. Fully Supported Motherboards (NCT5584D Super I/O)

Motherboards utilizing the **Nuvoton NCT5584D** Super I/O chip have native support for fan RPM monitoring, manual PWM control, 4-point SmartFan curve programming, and onboard temperature monitoring.

Below is the verified list extracted from `MBFanSensorHelper.NCT5584D_Datas` and `NCT5584D_DetailInfo`:

### AMD B850 / B840 / X870 Series (AM5)

- **CVN Series**:
    - CVN B850 GAMING PRO WIFI7 V14
    - CVN B850M ARK FROZEN V14
    - CVN B850M GAMING FROZEN V14
    - CVN B850I FROZEN WIFI V14
    - CVN B850I GAMING FROZEN V14
    - CVN X870 GAMING FROZEN V14
- **iGame / COLORFIRE**:
    - iGame B850I MINI OC V14
    - COLORFIRE B850M-A MEOW WIFI ORANGE
    - COLORFIRE B850M-MEOW WIFI7 V14
- **BATTLE-AX Series**:
    - BATTLE-AX B850M-PLUS PRO WIFI V14
    - BATTLE-AX B850M-PLUS WIFI V14
    - BATTLE-AX B850M-PLUS S WIFI7 V14
    - BATTLE-AX B850M-GHA WIFI V14
    - BATTLE-AX B850M-T WIFI V14
    - BATTLE-AX B850M-E WIFI V14
    - BATTLE-AX B840M-D PRO V14
    - BATTLE-AX B840M-GHA WIFI V14
    - BATTLE-AX X870A-GHA WIFI V14
- **iCafe Series**:
    - iCafe B850M-G DELUXE V14A
    - iCafe B850M-G DELUXE V15

### AMD B650 Series (AM5)

- **CVN Series**:
    - CVN B650 GAMING FROZEN V14 _(ATX)_
    - CVN B650M GAMING FROZEN V14 _(mATX)_
- **COLORFIRE**:
    - COLORFIRE B650M-MEOW WIFI ORANGE
- **BATTLE-AX Series**:
    - BATTLE-AX B650A-GHA WIFI V14 _(ATX)_
    - BATTLE-AX B650M-PLUS V14 / V15
    - BATTLE-AX B650M-PLUS WIFI V15
    - BATTLE-AX B650M-WHITE WIFI V14 / V15
    - BATTLE-AX B650M-A PLUS V14
    - BATTLE-AX B650M-D PRO V14
    - BATTLE-AX B650M-E WIFI V14
    - BATTLE-AX B650M-E PRO V14
    - BATTLE-AX B650M-GHA WIFI V14
    - BATTLE-AX B650M-T V14 / WIFI V14
- **iCafe Series**:
    - iCafe B650M-G DELUXE V14 / V14A / V15
    - iCafe B650M-PLUS DELUXE V14

### AMD A620 / A520 Series (AM5 / AM4)

- BATTLE-AX A620M-GHA WIFI V14
- BATTLE-AX A620M-D PRO V14
- BATTLE-AX A620AM-GHA WIFI V14
- BATTLE-AX A620AM-D PRO V14
- BATTLE-AX A520M-T WIFI V15

---

## 3. Fan Header Mapping & Bank Allocation

The NCT5584D physically provisions up to **5 fan channels**. Different board form factors wire and label these banks based on PCB space and silk screen layout.

### Bank & Register Reference

| Super I/O Bank | NCT5584D Channel | RPM Register   | `colorctl` Enum Target |
| -------------- | ---------------- | -------------- | ---------------------- |
| **Bank 2**     | `CPUFan`         | `194` (`0xC2`) | `FanHeader::Cpu`       |
| **Bank 1**     | `SYSFan`         | `192` (`0xC0`) | `FanHeader::ChaFan1`   |
| **Bank 10**    | `AUXFan3`        | `202` (`0xCA`) | `FanHeader::ChaFan2`   |
| **Bank 3**     | `AUXFan0`        | `196` (`0xC4`) | `FanHeader::ChaFan3`   |
| **Bank 11**    | `AUXFan4`        | `206` (`0xCE`) | `FanHeader::Pump`      |

### Board Wiring Variations

1. **Full 5-Header Boards (e.g. CVN B650 GAMING FROZEN V14, CVN B650M GAMING FROZEN V14, B650M-PLUS)**:
    - **ATX (CVN B650 GAMING FROZEN V14)**:
        - Bank 2 &rarr; `CPU_FAN1`
        - Bank 10 &rarr; `CPU_FAN2` _(Mapped to `ChaFan2` in colorctl)_
        - Bank 1 &rarr; `CHA_FAN1`
        - Bank 3 &rarr; `CHA_FAN2` _(Mapped to `ChaFan3` in colorctl)_
        - Bank 11 &rarr; `AIO_PUMP`
    - **mATX (CVN B650M GAMING FROZEN V14)**:
        - Bank 2 &rarr; `CPU_FAN`
        - Bank 1 &rarr; `CHA_FAN1`
        - Bank 10 &rarr; `CHA_FAN2`
        - Bank 3 &rarr; `CHA_FAN3`
        - Bank 11 &rarr; `AIO_PUMP`

2. **4-Header Boards (e.g. B650M-E, B650M-D PRO, A620M-GHA)**:
    - Only banks 1, 2, 3, and 11 are physically wired (`CPU_FAN`, `CHA_FAN1`, `CHA_FAN2`, `AIO_PUMP`). Bank 10 is unpopulated.

3. **Compact / ITX Boards (e.g. CVN B850I FROZEN WIFI, B650M-T)**:
    - 2 or 3 headers wired (typically `CPU_FAN`, `CHA_FAN`, and `AIO_PUMP`). Unpopulated banks simply return 0 RPM.

---

## 4. RGB Header Architecture & Differences

### Official Driver Behavior (Dynamic)

In the official software (`iGameMBoard.dll`), RGB configuration is queried dynamically from the microcontroller:

- The driver retrieves an `iGameLEDDevice_Buffer` with a dynamic `ChannelCount`.
- Channels are enumerated by index:
    - `0`: `MBoard_LED` (onboard RGB)
    - `121`, `122`: `12V_1`, `12V_2` (12V 4-pin RGB headers)
    - `51` through `58`: `5V_1` through `5V_8` (5V 3-pin ARGB headers)

### `colorctl` Implementation (Static Buffer)

`colorctl` employs a static 200-LED payload structure:

| Channel          | Label in `colorctl`        | Official ID        | LED Index Range | LED Count |
| ---------------- | -------------------------- | ------------------ | --------------- | --------- |
| `Channel::Led`   | `LED (Onboard)`            | `0` (`MBoard_LED`) | `0..18`         | 18        |
| `Channel::Rgb1`  | `12V_1 (12V RGB Header 1)` | `121` (`12V_1`)    | `18..19`        | 1         |
| `Channel::Rgb2`  | `12V_2 (12V RGB Header 2)` | `122` (`12V_2`)    | `19..20`        | 1         |
| `Channel::Argb1` | `5V_1 (ARGB Header 1)`     | `51` (`5V_1`)      | `20..80`        | 60        |
| `Channel::Argb2` | `5V_2 (ARGB Header 2)`     | `52` (`5V_2`)      | `80..140`       | 60        |
| `Channel::Argb3` | `5V_3 (ARGB Header 3)`     | `53` (`5V_3`)      | `140..200`      | 60        |

- **Hardware Compatibility**:
    - Boards with fewer physical headers (such as mATX or budget models lacking a third ARGB header or second 12V header) operate without issues: the MCU receives data for all slots, but inactive channels have no traces connecting to physical header pins.
    - Motherboards featuring the `0x2F4C:0x1000` USB HID device will respond to lighting controls even if their Super I/O chip differs.

---

## 5. Unsupported Super I/O Chips (Official Software Breakdown)

The official Colorful suite uses several different Super I/O implementations that are not currently supported by `colorctl`'s fan control:

| Super I/O Chip                   | Typical Chipset / Platform                    | Example Motherboards                                                                     |
| -------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------------------- |
| **Nuvoton NCT5585D**             | Intel 600/700/800 Series (12th–15th Gen Core) | CVN Z790/B760/Z890, BATTLE-AX B760M/H610M, iGame Z890M ULTRA (124 models in total)       |
| **Nuvoton NCT6796D / NCT6796DS** | High-end Enthusiast & Flagship                | iGame Z890 VULCAN/FLOW, iGame Z790D5 VULCAN, iGame X870E VULCAN OC, CVN X870E ARK FROZEN |
| **Nuvoton NCT6793D**             | Older AM4                                     | CVN B550M GAMING FROZEN V15                                                              |
| **Nuvoton NCT5567D_B**           | Entry AM4                                     | BATTLE-AX B550M-T PRO V14, BATTLE-AX B550M-D PRO V14                                     |
| **ITE IT8613E**                  | Select Budget Intel                           | BATTLE-AX H610M-GHA WIFI D5 V20                                                          |
