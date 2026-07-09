# Packaging: brightness daemon + DE-OSD bridge

`msi-klc daemon` bridges the tool's **software-owned** keyboard brightness to
your desktop's native keyboard-backlight OSD (the same slider GNOME/KDE show
for laptop keyboard backlights), using the kernel `uleds` userspace-LED
module and UPower.

## How it works

1. The daemon registers a `uleds` device named `msiklc::kbd_backlight`
   (`max_brightness` 255). This creates
   `/sys/class/leds/msiklc::kbd_backlight`.
2. UPower adopts any `*::kbd_backlight` LED under its
   `org.freedesktop.UPower.KbdBacklight` D-Bus interface. GNOME/KDE then show
   the native keyboard-backlight OSD for free.
3. When you change brightness from the DE, UPower writes the new value; the
   kernel delivers it to the daemon's `read(/dev/uleds)`. The daemon clamps it
   to `0..=255`, stores it, and **re-applies the last logical lighting frame**
   scaled by the new brightness (brightness is a software multiplier on the
   RGB we send — the keyboard's firmware/EC brightness path is unreliable on
   this hardware, so we own brightness in software).

`msi-klc brightness <VALUE>` from the CLI does the same re-apply and also
mirrors the raw value into `/sys/class/leds/msiklc::kbd_backlight/brightness`,
so the OSD stays in sync with CLI changes.

## Install

```sh
# 1. Build + install the binary (adjust prefix to taste).
cargo build --release
sudo install -Dm755 target/release/msi-klc /usr/local/bin/msi-klc

# 2. Ensure the uleds module is loaded now and on every boot.
sudo modprobe uleds
echo uleds | sudo tee /etc/modules-load.d/msi-klc.conf

# 3. Install and enable the systemd unit.
sudo install -Dm644 packaging/msi-klc-daemon.service \
    /etc/systemd/system/msi-klc-daemon.service
sudo systemctl daemon-reload
sudo systemctl enable --now msi-klc-daemon.service

# 4. Restart UPower so it enumerates the freshly-registered LED.
sudo systemctl restart upower
```

## Ordering constraint (important)

**UPower only enumerates `*::kbd_backlight` LEDs at its own startup — there is
no hot-add.** So:

- The systemd unit is ordered `Before=upower.service` (and
  `WantedBy=multi-user.target`) so the LED exists before UPower starts at
  boot.
- If you start the daemon *after* UPower is already running (e.g. the very
  first time you install it), you must `systemctl restart upower` once, as in
  step 4 above. After that, boot ordering handles it.

## Verify

```sh
ls -l /sys/class/leds/msiklc::kbd_backlight        # LED node exists
gdbus call --system --dest org.freedesktop.UPower \
  --object-path /org/freedesktop/UPower/KbdBacklight \
  --method org.freedesktop.UPower.KbdBacklight.GetMaxBrightness   # -> 255
```

Then use your keyboard-backlight OSD keys, or:

```sh
msi-klc all 00FF00        # set something visible first (stores a frame)
msi-klc brightness 50%    # dim it; OSD updates too
```

## Uninstall

```sh
sudo systemctl disable --now msi-klc-daemon.service
sudo rm /etc/systemd/system/msi-klc-daemon.service /etc/modules-load.d/msi-klc.conf
sudo systemctl daemon-reload
```

Stopping the daemon closes `/dev/uleds`, which removes the LED node.
