//! `msi-klc daemon` — bridge the software-owned keyboard brightness to the
//! desktop environment's native backlight OSD via the `uleds` userspace-LED
//! kernel module.
//!
//! Validated end-to-end on the MS-16V5 / GS66: registering a `uleds` device
//! whose name contains `kbd_backlight` (we use `msiklc::kbd_backlight`,
//! `max_brightness` 255) creates `/sys/class/leds/msiklc::kbd_backlight`, and
//! after UPower (re)starts it adopts that LED under
//! `org.freedesktop.UPower.KbdBacklight`. GNOME/KDE then render the native
//! keyboard-backlight OSD for free, and each `SetBrightness` from the DE
//! arrives here as a 4-byte native-endian int from `read(/dev/uleds)`.
//!
//! Deployment note (see `packaging/`): UPower only enumerates `kbd_backlight`
//! LEDs at **its** startup — there's no hot-add — so the systemd unit must be
//! ordered `Before=upower.service`.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use crate::device::Device;
use crate::layout::Layout;
use crate::{apply_frame, state};

/// LED name we register. Must contain `kbd_backlight` for UPower to treat it
/// as the keyboard backlight (`upower`'s `up_kbd_backlight_find`).
const ULEDS_LED_NAME: &str = "msiklc::kbd_backlight";

/// `LED_MAX_NAME_SIZE` from the kernel `uleds` ABI: the `name[]` field of
/// `struct uleds_user_dev` is 64 bytes, NUL-padded.
const LED_MAX_NAME_SIZE: usize = 64;

/// Size of `struct uleds_user_dev`: `char name[64]` + `__u32 max_brightness`,
/// no padding (the u32 is naturally 4-aligned after 64 bytes).
const ULEDS_USER_DEV_LEN: usize = LED_MAX_NAME_SIZE + 4;

/// The `brightness` sysfs attribute UPower/the DE ultimately drive. We mirror
/// CLI `brightness` writes here so the OSD stays in sync (the daemon itself
/// reads *from* `/dev/uleds`, it does not write this).
pub const LED_SYSFS_BRIGHTNESS: &str = "/sys/class/leds/msiklc::kbd_backlight/brightness";

/// Set by the SIGINT/SIGTERM handler so the read loop can exit and drop the
/// `/dev/uleds` fd (closing it removes the LED cleanly).
static TERMINATE: AtomicBool = AtomicBool::new(false);

extern "C" fn on_terminate(_sig: i32) {
    TERMINATE.store(true, Ordering::SeqCst);
}

/// Install SIGINT/SIGTERM handlers *without* `SA_RESTART`, so a blocking
/// `read()` on `/dev/uleds` is interrupted (EINTR) and the loop can notice
/// the terminate flag.
fn install_signal_handlers() -> Result<()> {
    let action = SigAction::new(SigHandler::Handler(on_terminate), SaFlags::empty(), SigSet::empty());
    // SAFETY: `on_terminate` only touches an atomic; safe as a signal handler.
    unsafe {
        sigaction(Signal::SIGINT, &action).context("installing SIGINT handler")?;
        sigaction(Signal::SIGTERM, &action).context("installing SIGTERM handler")?;
    }
    Ok(())
}

/// Register the `uleds` device: open `/dev/uleds` O_RDWR and write the
/// 68-byte `struct uleds_user_dev`. The LED exists until the returned `File`
/// is dropped.
fn register_uleds(max_brightness: u32) -> Result<File> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uleds")
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "/dev/uleds not found — load the uleds kernel module first: `sudo modprobe uleds` \
                     (and add `uleds` to /etc/modules-load.d/ to persist). Underlying error: {e}"
                )
            } else {
                anyhow::anyhow!("opening /dev/uleds (needs root): {e}")
            }
        })?;

    let mut buf = [0u8; ULEDS_USER_DEV_LEN];
    let name = ULEDS_LED_NAME.as_bytes();
    // Name must fit with room for the NUL terminator the kernel expects.
    if name.len() >= LED_MAX_NAME_SIZE {
        bail!("uleds name {ULEDS_LED_NAME:?} too long");
    }
    buf[..name.len()].copy_from_slice(name);
    buf[LED_MAX_NAME_SIZE..].copy_from_slice(&max_brightness.to_ne_bytes());
    file.write_all(&buf)
        .context("registering uleds device (writing struct uleds_user_dev)")?;
    Ok(file)
}

/// Best-effort mirror of the raw brightness (0..=255) into the LED's sysfs
/// `brightness` attribute, so DE OSD/UPower state matches after a CLI change.
/// A no-op (with a note) if the daemon isn't running / the attribute is
/// absent, and errors are non-fatal.
pub fn sync_led_sysfs(brightness: u8) {
    if Path::new(LED_SYSFS_BRIGHTNESS).exists() {
        if let Err(e) = std::fs::write(LED_SYSFS_BRIGHTNESS, format!("{brightness}\n")) {
            eprintln!("msi-klc: could not update {LED_SYSFS_BRIGHTNESS}: {e}");
        }
    }
}

/// Run the daemon: register the uleds LED, then loop applying each brightness
/// value the DE sends. `max_brightness` is what we advertise to userspace
/// (default 255); received values are clamped to 0..=255 regardless, since
/// our internal brightness space is fixed at 0..=255.
pub fn run(dev: &Device, layout: &Layout, color_scale: [f32; 3], max_brightness: u32) -> Result<()> {
    install_signal_handlers()?;
    let mut uleds = register_uleds(max_brightness)?;
    eprintln!(
        "msi-klc: registered uleds LED {ULEDS_LED_NAME:?} (max_brightness {max_brightness}). \
         Restart UPower to pick it up (systemd orders us Before=upower.service). Ctrl-C to stop."
    );

    let mut ev = [0u8; 4];
    loop {
        match uleds.read(&mut ev) {
            Ok(4) => {
                let raw = i32::from_ne_bytes(ev);
                let brightness = raw.clamp(0, 255) as u8;
                if let Err(e) = apply_brightness(dev, layout, color_scale, brightness) {
                    eprintln!("msi-klc: applying brightness {brightness} failed: {e}");
                }
            }
            Ok(0) => {
                eprintln!("msi-klc: /dev/uleds returned EOF, exiting");
                break;
            }
            Ok(n) => eprintln!("msi-klc: ignoring short uleds read ({n} bytes)"),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                if TERMINATE.load(Ordering::SeqCst) {
                    break;
                }
                // Spurious EINTR (not our terminate signal): retry the read.
            }
            Err(e) => return Err(e).context("reading /dev/uleds"),
        }
    }

    eprintln!("msi-klc: removing uleds LED and exiting");
    // Dropping `uleds` closes the fd, which removes the LED.
    drop(uleds);
    Ok(())
}

/// Shared re-apply path (also used by `brightness <VALUE>`): store the new
/// brightness and re-render the last logical frame folded at that brightness.
pub fn apply_brightness(
    dev: &Device,
    layout: &Layout,
    color_scale: [f32; 3],
    brightness: u8,
) -> Result<()> {
    let mut st = state::load();
    st.brightness = brightness;
    if let Some(frame) = st.frame.clone() {
        let scale = state::fold_brightness(color_scale, brightness);
        apply_frame(dev, layout, &frame, scale)?;
    }
    state::save(&st)?;
    Ok(())
}
