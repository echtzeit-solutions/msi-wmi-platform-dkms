#!/bin/bash
# Visually blink each LED so a human can confirm the physical indicator.
# Run as root. Detaches auto-triggers first so writes aren't overwritten.
L=/sys/class/leds
say() { echo; echo ">>> $1"; }

# make sure nothing auto-drives them
echo none > $L/platform::micmute/trigger 2>/dev/null
echo none > $L/platform::mute/trigger 2>/dev/null

say "MIC-MUTE LED (mic key) -> ON for 4s"
echo 1 > $L/platform::micmute/brightness; sleep 4
echo "   -> OFF"; echo 0 > $L/platform::micmute/brightness; sleep 1

say "MUTE LED (speaker mute) -> ON for 4s"
echo 1 > $L/platform::mute/brightness; sleep 4
echo "   -> OFF"; echo 0 > $L/platform::mute/brightness; sleep 1

say "USB BACKLIGHT -> ramp 0..255 (2s each), watch USB ports"
for b in 64 128 192 255; do echo "   b=$b"; echo $b > $L/msi::usb_backlight/brightness; sleep 2; done
echo "   -> OFF"; echo 0 > $L/msi::usb_backlight/brightness; sleep 1

say "KBD BACKLIGHT ENABLE -> ON 4s (watch keyboard), then OFF"
echo 1 > $L/msi::kbd_backlight/brightness; sleep 4
echo "   -> OFF"; echo 0 > $L/msi::kbd_backlight/brightness; sleep 1

# three quick blinks of each status LED to make them unmistakable
say "FINALE: 3 fast blinks of mic-mute then mute"
for i in 1 2 3; do echo 1 > $L/platform::micmute/brightness; sleep .25; echo 0 > $L/platform::micmute/brightness; sleep .25; done
for i in 1 2 3; do echo 1 > $L/platform::mute/brightness; sleep .25; echo 0 > $L/platform::mute/brightness; sleep .25; done

echo; echo "BLINK_DONE"
