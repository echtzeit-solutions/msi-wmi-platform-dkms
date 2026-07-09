//! hidraw device discovery + I/O for the MSI KLC keyboard controller.
//!
//! Detection scans `/sys/class/hidraw/*/device/uevent` for a `HID_ID` line
//! matching `0003:0000<VID>:0000<PID>` (bus=USB), which is how the kernel
//! exposes VID/PID for hidraw nodes without needing libudev. This mirrors
//! what `msi-nb-rgb.py`/`klc-cmd.py` assume when the caller passes
//! `--device /dev/hidrawN` — we just add the ability to find N automatically.

use anyhow::{Context, Result, bail};
use nix::ioctl_readwrite;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::protocol::{FEATURE_REPORT_LEN, OUTPUT_REPORT_LEN};

pub const VID_MSI: u16 = 0x1038;

// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len). The ioctl
// number bakes in the buffer size, so we generate one wrapper sized exactly
// to our fixed 525-byte feature report (matches `msi-nb-rgb.py`'s
// `HIDIOCSFEATURE(len(data))` where `len(data)` is always `REPORT_LEN`).
ioctl_readwrite!(hidiocsfeature_525, b'H', 0x06, [u8; FEATURE_REPORT_LEN]);

/// Find a hidraw node under `/sys/class/hidraw` whose `HID_ID` matches
/// `vid` and (if given) one of `pids`. Returns the first match as
/// `(/dev/hidrawN, matched_pid)` — the matched PID lets the caller resolve
/// which model (of possibly many sharing `vid`) it just found, via
/// `models::ModelTable::find`.
pub fn find_device(vid: u16, pids: Option<&[u16]>) -> Result<(PathBuf, u16)> {
    let dir = fs::read_dir("/sys/class/hidraw").context("reading /sys/class/hidraw (is hidraw loaded?)")?;
    for entry in dir {
        let entry = entry?;
        let uevent_path = entry.path().join("device/uevent");
        let Ok(content) = fs::read_to_string(&uevent_path) else {
            continue;
        };
        let Some((found_vid, found_pid)) = parse_hid_id(&content) else {
            continue;
        };
        if found_vid == vid && pids.is_none_or(|list| list.contains(&found_pid)) {
            return Ok((PathBuf::from("/dev").join(entry.file_name()), found_pid));
        }
    }
    let pid_desc = match pids {
        None => "any".to_string(),
        Some([p]) => format!("0x{p:04x}"),
        Some(list) => format!("one of {} known KLC PIDs", list.len()),
    };
    bail!(
        "no /dev/hidraw* device found for vid=0x{vid:04x} pid={pid_desc}. Pass --path /dev/hidrawN \
         explicitly, or check `grep HID_ID /sys/class/hidraw/*/device/uevent`."
    )
}

/// Try to resolve the `(vid, pid)` of an already-known hidraw device path
/// (e.g. one given via `--path`) by reading back its sysfs `uevent`, the
/// same way `find_device` does for auto-detection. Returns `None` if the
/// path isn't a `/dev/hidrawN`-shaped node or its uevent can't be read —
/// callers should treat that as "model unknown", not a hard error.
pub fn probe_vid_pid(path: &Path) -> Option<(u16, u16)> {
    let name = path.file_name()?.to_str()?;
    let uevent_path = PathBuf::from("/sys/class/hidraw").join(name).join("device/uevent");
    let content = fs::read_to_string(uevent_path).ok()?;
    parse_hid_id(&content)
}

/// Parse `HID_ID=0003:00001038:0000113A` into `(vid, pid)`. The bus type
/// (0003 = USB) is ignored; only the low 16 bits of the vendor/product
/// fields carry the actual VID/PID.
fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    let line = uevent.lines().find(|l| l.starts_with("HID_ID="))?;
    let rest = line.strip_prefix("HID_ID=")?;
    let mut parts = rest.split(':');
    let _bus = parts.next()?;
    let vid_field = parts.next()?;
    let pid_field = parts.next()?;
    let vid = u32::from_str_radix(vid_field, 16).ok()? as u16;
    let pid = u32::from_str_radix(pid_field, 16).ok()? as u16;
    Some((vid, pid))
}

/// An open hidraw device.
pub struct Device {
    file: File,
}

impl Device {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("opening {} (needs root)", path.display()))?;
        Ok(Self { file })
    }

    /// Send a 525-byte feature report via `HIDIOCSFEATURE`, exactly like
    /// `msi-nb-rgb.py:send_feature`.
    pub fn send_feature(&self, buf: &[u8; FEATURE_REPORT_LEN]) -> Result<()> {
        let mut buf = *buf;
        unsafe { hidiocsfeature_525(self.file.as_raw_fd(), &mut buf as *mut _) }
            .context("HIDIOCSFEATURE ioctl failed")?;
        Ok(())
    }

    /// Write an output report (vendor command or commit/"show" frame) via a
    /// plain `write()`, like `klc-cmd.py:send_output` / `refresh()`.
    pub fn write_output(&self, buf: &[u8]) -> Result<()> {
        (&self.file).write_all(buf).context("writing output report")?;
        Ok(())
    }

    /// Best-effort drain of any stale input report, non-blocking.
    pub fn drain_input(&self) -> Result<()> {
        while self.read_input(Duration::from_millis(50))?.is_some() {}
        Ok(())
    }

    /// Read one input report (GET reply), waiting up to `timeout` via
    /// `poll()`. Returns `None` on timeout, like `klc-cmd.py:read_reply`.
    pub fn read_input(&self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        let fd = self.file.as_fd();
        let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
        let timeout_ms: i32 = timeout.as_millis().try_into().unwrap_or(i32::MAX);
        let n = poll(&mut fds, PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::MAX))
            .context("poll() on hidraw fd")?;
        if n == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; OUTPUT_REPORT_LEN];
        let read = (&self.file).read(&mut buf).context("reading input report")?;
        buf.truncate(read);
        Ok(Some(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hid_id_uevent_line() {
        let uevent = "DRIVER=hid-generic\nHID_ID=0003:00001038:0000113A\nHID_NAME=foo\n";
        assert_eq!(parse_hid_id(uevent), Some((0x1038, 0x113a)));
    }

    #[test]
    fn missing_hid_id_returns_none() {
        assert_eq!(parse_hid_id("DRIVER=hid-generic\n"), None);
    }
}
