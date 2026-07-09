#!/usr/bin/env python3
"""De-risk the DE-OSD chain: register a userspace LED named '*::kbd_backlight'
via /dev/uleds and hold it open, printing brightness values the kernel/DE
write back. If GNOME/KDE's power daemon (via UPower) picks it up, the
keyboard-backlight OSD will fire when the slider/media key changes it.

uleds ABI: write struct { char name[64]; __u32 max_brightness } (LED_MAX_NAME_SIZE=64),
then read() returns a 4-byte int brightness on each change.
Run as root:  sudo ./uleds-osd-test.py
"""
import os, struct, select, sys

NAME = b"msiklc::kbd_backlight"
MAX = 255

fd = os.open("/dev/uleds", os.O_RDWR)
os.write(fd, struct.pack("64sI", NAME, MAX))   # 64-byte name + u32 max_brightness
print(f"registered uleds '{NAME.decode()}' max_brightness={MAX}; holding open. Ctrl-C to remove.")
sys.stdout.flush()
try:
    while True:
        r, _, _ = select.select([fd], [], [], 30)
        if r:
            val = struct.unpack("i", os.read(fd, 4))[0]
            print(f"DE/kernel set brightness -> {val}", flush=True)
finally:
    os.close(fd)
