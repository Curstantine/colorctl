use std::fs::{self, OpenOptions};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[inline]
/// Converts a percentage (0-100) to a PWM value (0-255)
pub const fn pct(p: u8) -> u8 {
    let p = if p > 100 { 100 } else { p };
    ((p as u32 * 255 + 99) / 100) as u8
}

#[inline]
pub fn is_root() -> bool {
    fs::metadata("/proc/self")
        .map(|m| m.uid() == 0)
        .unwrap_or(false)
}

pub fn resolve_state_path(filename: &str) -> PathBuf {
    if let Ok(path) = std::env::var("COLORCTL_STATE_FILE") {
        return PathBuf::from(path);
    }

    if let Ok(dir) = std::env::var("COLORCTL_STATE_DIR") {
        return PathBuf::from(dir).join(filename);
    }

    let sys_dir = Path::new("/var/lib/colorctl");
    let sys_file = sys_dir.join(filename);

    if sys_file.is_file() {
        if OpenOptions::new().write(true).open(&sys_file).is_ok() {
            return sys_file;
        }
    }

    // 2. If running as root, ensure /var/lib/colorctl exists (0777) and use sys_file
    if is_root() {
        if fs::create_dir_all(sys_dir).is_ok() {
            let _ = fs::set_permissions(sys_dir, fs::Permissions::from_mode(0o777));
            return sys_file;
        }
    }

    // 3. If /var/lib/colorctl directory exists and is writable by non-root, use sys_file
    if sys_dir.is_dir() {
        if OpenOptions::new()
            .create(true)
            .write(true)
            .open(&sys_file)
            .is_ok()
        {
            let _ = fs::set_permissions(&sys_file, fs::Permissions::from_mode(0o666));
            return sys_file;
        }
    }

    // 4. Fallback to user state directory ($XDG_STATE_HOME or ~/.local/state)
    let user_state_dir = if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg).join("colorctl")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/state/colorctl")
    } else {
        PathBuf::from("/tmp/colorctl")
    };

    user_state_dir.join(filename)
}

pub fn ensure_dir_permissions(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        if is_root() && parent == Path::new("/var/lib/colorctl") {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o777));
        }
    }
    Ok(())
}

pub fn set_world_writable(path: &Path) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o666))
}
