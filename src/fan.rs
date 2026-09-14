use anyhow::{Result, bail};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub const NCT5584D_CHIP_ID: u16 = 0xd42a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanHeader {
    Cpu,  // CPU_FAN (Bank 2)
    Sys1, // CHA_FAN1 (Bank 1)
    Sys2, // CHA_FAN2 (Bank 10 / 0x0A)
    Sys3, // CHA_FAN3 (Bank 3)
    Pump, // AIO_PUMP (Bank 11 / 0x0B)
}

impl FanHeader {
    pub fn name(&self) -> &'static str {
        match self {
            FanHeader::Cpu => "CPU_FAN",
            FanHeader::Sys1 => "CHA_FAN1",
            FanHeader::Sys2 => "CHA_FAN2",
            FanHeader::Sys3 => "CHA_FAN3",
            FanHeader::Pump => "AIO_PUMP",
        }
    }

    pub fn bank(&self) -> u8 {
        match self {
            FanHeader::Cpu => 2,
            FanHeader::Sys1 => 1,
            FanHeader::Sys2 => 10,
            FanHeader::Sys3 => 3,
            FanHeader::Pump => 11,
        }
    }

    pub fn rpm_reg(&self) -> u8 {
        match self {
            FanHeader::Sys1 => 192, // 0xC0
            FanHeader::Cpu => 194,  // 0xC2
            FanHeader::Sys3 => 196, // 0xC4
            FanHeader::Sys2 => 202, // 0xCA
            FanHeader::Pump => 206, // 0xCE
        }
    }

    pub fn all() -> &'static [FanHeader] {
        &[
            FanHeader::Cpu,
            FanHeader::Sys1,
            FanHeader::Sys2,
            FanHeader::Sys3,
            FanHeader::Pump,
        ]
    }

    pub fn from_str(s: &str) -> Option<FanHeader> {
        match s.to_ascii_lowercase().as_str() {
            "cpu" | "cpu_fan" | "cpufan" => Some(FanHeader::Cpu),
            "sys1" | "cha_fan1" | "chafan1" | "sys_fan1" | "sys" => Some(FanHeader::Sys1),
            "sys2" | "cha_fan2" | "chafan2" | "sys_fan2" => Some(FanHeader::Sys2),
            "sys3" | "cha_fan3" | "chafan3" | "sys_fan3" => Some(FanHeader::Sys3),
            "pump" | "aio_pump" | "aiopump" => Some(FanHeader::Pump),
            _ => None,
        }
    }
}

pub enum Backend {
    Direct,
    DevPort(File),
}

impl Backend {
    pub fn new() -> Result<Self> {
        unsafe {
            if libc::iopl(3) == 0 {
                return Ok(Backend::Direct);
            }
        }

        // Fallback to /dev/port
        match OpenOptions::new().read(true).write(true).open("/dev/port") {
            Ok(file) => Ok(Backend::DevPort(file)),
            Err(e) => {
                bail!(
                    "Port I/O access failed: {}. Root privileges (su/sudo or CAP_SYS_RAWIO) required.",
                    e
                );
            }
        }
    }

    pub fn read_byte(&mut self, port: u16) -> Result<u8> {
        match self {
            Backend::Direct => unsafe {
                let val: u8;
                std::arch::asm!(
                    "in al, dx",
                    in("dx") port,
                    out("al") val,
                    options(nomem, nostack, preserves_flags)
                );
                Ok(val)
            },
            Backend::DevPort(file) => {
                file.seek(SeekFrom::Start(port as u64))?;
                let mut buf = [0u8; 1];
                file.read_exact(&mut buf)?;
                Ok(buf[0])
            }
        }
    }

    pub fn write_byte(&mut self, port: u16, val: u8) -> Result<()> {
        match self {
            Backend::Direct => unsafe {
                std::arch::asm!(
                    "out dx, al",
                    in("dx") port,
                    in("al") val,
                    options(nomem, nostack, preserves_flags)
                );
                Ok(())
            },
            Backend::DevPort(file) => {
                file.seek(SeekFrom::Start(port as u64))?;
                file.write_all(&[val])?;
                Ok(())
            }
        }
    }
}

pub struct SuperIo {
    backend: Backend,
    #[allow(dead_code)]
    pub reg_port: u16,
    #[allow(dead_code)]
    pub val_port: u16,
    pub hwm_index_port: u16,
    pub hwm_data_port: u16,
    pub chip_id: u16,
}

impl SuperIo {
    pub fn open() -> Result<Self> {
        let mut backend = Backend::new()?;

        // Test port pairs: 0x4E/0x4F then 0x2E/0x2F
        let port_candidates = [(0x4Eu16, 0x4Fu16), (0x2Eu16, 0x2Fu16)];
        let mut selected = None;

        for (reg, val) in port_candidates {
            if let Ok(id) = Self::read_chip_id(&mut backend, reg, val) {
                if id == NCT5584D_CHIP_ID || (id & 0xFFF0) == (NCT5584D_CHIP_ID & 0xFFF0) {
                    selected = Some((reg, val, id));
                    break;
                }
            }
        }

        let (reg_port, val_port, chip_id) = match selected {
            Some(s) => s,
            None => {
                let id = Self::read_chip_id(&mut backend, 0x4E, 0x4F).unwrap_or(0);
                (0x4E, 0x4F, id)
            }
        };

        // Disable IO space lock
        Self::disable_io_space_lock(&mut backend, reg_port, val_port)?;

        // Query Hardware Monitor (HWM) Base Address from Logical Device 0x0B
        backend.write_byte(reg_port, 0x87)?; // Enter config mode
        backend.write_byte(reg_port, 0x87)?;
        backend.write_byte(reg_port, 0x07)?; // Select Logical Device Number
        backend.write_byte(val_port, 0x0B)?; // LDN 11 (Hardware Monitor)
        backend.write_byte(reg_port, 0x60)?; // Base Address MSB
        let base_hi = backend.read_byte(val_port)?;
        backend.write_byte(reg_port, 0x61)?; // Base Address LSB
        let base_lo = backend.read_byte(val_port)?;
        backend.write_byte(reg_port, 0xAA)?; // Exit config mode

        let hwm_base = ((base_hi as u16) << 8) | (base_lo as u16);
        let (hwm_index_port, hwm_data_port) = if hwm_base != 0 && hwm_base != 0xFFFF {
            (hwm_base + 5, hwm_base + 6)
        } else {
            (0x0A25, 0x0A26) // Fallback from Colorful decompiled code (2597 / 2598)
        };

        Ok(Self {
            backend,
            reg_port,
            val_port,
            hwm_index_port,
            hwm_data_port,
            chip_id,
        })
    }

    fn read_chip_id(backend: &mut Backend, reg_port: u16, val_port: u16) -> Result<u16> {
        backend.write_byte(reg_port, 0x87)?;
        backend.write_byte(reg_port, 0x87)?;
        backend.write_byte(reg_port, 0x20)?;
        let hi = backend.read_byte(val_port)?;
        backend.write_byte(reg_port, 0x21)?;
        let lo = backend.read_byte(val_port)?;
        backend.write_byte(reg_port, 0xAA)?;
        Ok(((hi as u16) << 8) | (lo as u16))
    }

    fn disable_io_space_lock(backend: &mut Backend, reg_port: u16, val_port: u16) -> Result<()> {
        backend.write_byte(reg_port, 0x87)?;
        backend.write_byte(reg_port, 0x87)?;
        backend.write_byte(reg_port, 0x28)?;
        let val = backend.read_byte(val_port)?;
        if (val & 0x10) != 0 {
            backend.write_byte(val_port, val & !0x10)?;
        }
        backend.write_byte(reg_port, 0xAA)?;
        Ok(())
    }

    pub fn set_bank(&mut self, bank: u8) -> Result<()> {
        self.backend.write_byte(self.hwm_index_port, 0x4E)?;
        let cur = self.backend.read_byte(self.hwm_data_port)?;
        self.backend
            .write_byte(self.hwm_data_port, (cur & 0xF0) | (bank & 0x0F))?;
        Ok(())
    }

    pub fn read_hwm_reg(&mut self, bank: u8, reg: u8) -> Result<u8> {
        self.set_bank(bank)?;
        self.backend.write_byte(self.hwm_index_port, reg)?;
        self.backend.read_byte(self.hwm_data_port)
    }

    pub fn write_hwm_reg(&mut self, bank: u8, reg: u8, val: u8) -> Result<()> {
        self.set_bank(bank)?;
        self.backend.write_byte(self.hwm_index_port, reg)?;
        self.backend.write_byte(self.hwm_data_port, val)?;
        Ok(())
    }

    pub fn get_fan_rpm(&mut self, header: FanHeader) -> Result<u16> {
        let base_reg = header.rpm_reg();
        let hi = self.read_hwm_reg(4, base_reg)?;
        let lo = self.read_hwm_reg(4, base_reg + 1)?;
        let rpm = ((hi as u16) << 8) | (lo as u16);
        if rpm >= 50000 { Ok(0) } else { Ok(rpm) }
    }

    pub fn get_temperature(&mut self) -> Result<u8> {
        let temp = self.read_hwm_reg(1, 0x50)?;
        if temp >= 150 { Ok(0) } else { Ok(temp) }
    }

    pub fn set_fan_speed_pwm(&mut self, header: FanHeader, pwm: u8) -> Result<()> {
        let bank = header.bank();
        // Register 2: 0x00 = Manual Mode
        self.write_hwm_reg(bank, 0x02, 0x00)?;
        // Register 9: PWM duty (0..255)
        self.write_hwm_reg(bank, 0x09, pwm)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_fan_speed_percent(&mut self, header: FanHeader, percent: u8) -> Result<()> {
        let percent = percent.min(100);
        let pwm = ((percent as f64) * 2.55).ceil() as u8;
        self.set_fan_speed_pwm(header, pwm)
    }

    pub fn set_fan_curve(&mut self, header: FanHeader, points: &[(u8, u8); 4]) -> Result<()> {
        let bank = header.bank();
        // Configure SmartFan mode according to NCT5584D_Service.cs:
        self.write_hwm_reg(bank, 0x66, 0xAA)?; // 170
        self.write_hwm_reg(bank, 0x03, 0x01)?;
        self.write_hwm_reg(bank, 0x04, 0x01)?;
        self.write_hwm_reg(bank, 0x02, 0x40)?; // 64 (SmartFan mode)

        // 4 Temperature points (°C): registers 0x21, 0x22, 0x23, 0x24 (33, 34, 35, 36)
        for (i, (temp_c, _)) in points.iter().enumerate() {
            self.write_hwm_reg(bank, 0x21 + (i as u8), *temp_c)?;
        }

        // 4 PWM points (0..255): registers 0x27, 0x28, 0x29, 0x2A (39, 40, 41, 42)
        for (i, (_, pwm)) in points.iter().enumerate() {
            self.write_hwm_reg(bank, 0x27 + (i as u8), *pwm)?;
        }

        Ok(())
    }
}
