#!/bin/bash
# Objective hardware sweep for msi-wmi-platform dev build (LEDs + camera + lockdown).
# Run as root:  sudo bash sweep.sh
# Reads EC bytes via ec_sys to prove each write reaches the EC. Does NOT judge the
# physical effect (that's your eyes) -- it fills the "EC toggles?" column only.
set -u
KO=/home/florian/src-laptop/linux-msi-ms16v5/msi-wmi-platform/msi-wmi-platform.ko
L=/sys/class/leds
IO=/sys/kernel/debug/ec/ec0/io
pass=0 fail=0
ec() { python3 -c "print('0x%02X'%open('$IO','rb').read()[$1])"; }
chk() { # name reg expect actual
  if [ "$3" = "$4" ]; then echo "  OK   $1: reg $2 = $4 (want $3)"; pass=$((pass+1));
  else echo "  FAIL $1: reg $2 = $4 (want $3)"; fail=$((fail+1)); fi
}

echo "=== reload dev module ==="
rmmod msi_wmi_platform 2>/dev/null
insmod "$KO" || { echo "insmod FAILED"; exit 1; }
modprobe ec_sys 2>/dev/null
echo "srcversion: $(modinfo -F srcversion $KO)"
dmesg | grep -i msi_wmi_platform | tail -6
echo
echo "LEDs present: $(ls $L | grep -Ei 'micmute|mute|kbd_backlight|usb_backlight' | tr '\n' ' ')"
CP=$(find /sys -name camera_power 2>/dev/null); echo "camera_power: ${CP:-<absent>}"
echo

echo "=== 1a mic-mute LED (0x2C.0) ==="
echo none > $L/platform::micmute/trigger 2>/dev/null
echo 1 > $L/platform::micmute/brightness; a1=$(ec 0x2C)
echo 0 > $L/platform::micmute/brightness; a0=$(ec 0x2C)
chk micmute-on 0x2C 0x01 "$a1"; chk micmute-off 0x2C 0x00 "$a0"

echo "=== 1b mute LED (0x2D.0) ==="
# reg 0x2D holds other firmware-owned bits (bit2 seen set); check bit0 only
echo none > $L/platform::mute/trigger 2>/dev/null
echo 1 > $L/platform::mute/brightness; a1=$(ec 0x2D)
echo 0 > $L/platform::mute/brightness; a0=$(ec 0x2D)
b1=$(( 0x${a1#0x} & 1 )); b0=$(( 0x${a0#0x} & 1 ))
chk mute-on-bit0 0x2D 1 "$b1"; chk mute-off-bit0 0x2D 0 "$b0"

echo "=== 1c USB backlight (0xDB, byte) ==="
for b in 0x00 0x40 0x80 0xFF; do
  echo $((b)) > $L/msi::usb_backlight/brightness; chk usb-$b 0xDB "$b" "$(ec 0xDB)"
done
echo 0 > $L/msi::usb_backlight/brightness

echo "=== 1d kbd-backlight enable (0xEC.1) ==="
b=$(ec 0xEC); echo "  baseline 0xEC = $b"
echo 1 > $L/msi::kbd_backlight/brightness; on=$(ec 0xEC)
echo 0 > $L/msi::kbd_backlight/brightness; off=$(ec 0xEC)
# bit1: on should have bit set, off clear
onbit=$(( (0x${on#0x} >> 1) & 1 )); offbit=$(( (0x${off#0x} >> 1) & 1 ))
chk kbd-on-bit1 0xEC 1 "$onbit"; chk kbd-off-bit1 0xEC 0 "$offbit"

echo "=== 2 camera_power (0x2E.1) ==="
if [ -n "$CP" ]; then
  echo "  start: cat=$(cat $CP)  ec0x2E=$(ec 0x2E)  video=$(ls /dev/video* 2>/dev/null | tr '\n' ' ')  usb=$(lsusb | grep -i 5986:2127)"
  echo 1 > $CP; sleep 3
  echo "  ON   : cat=$(cat $CP)  ec0x2E=$(ec 0x2E)  video=$(ls /dev/video* 2>/dev/null | tr '\n' ' ')  usb=$(lsusb | grep -i 5986:2127)"
  echo 0 > $CP; sleep 2
  echo "  OFF  : cat=$(cat $CP)  ec0x2E=$(ec 0x2E)  video=$(ls /dev/video* 2>/dev/null | tr '\n' ' ')  usb=$(lsusb | grep -i 5986:2127)"
  echo 1 > $CP   # restore to powered-on (device present) state
else
  echo "  camera_power absent -> SupportedWebCam bit not set on this probe"
fi

echo "=== 3 lockdown baseline ==="
echo "  lockdown: $(cat /sys/kernel/security/lockdown)"
SE=$(find /sys/kernel/debug -name set_ec 2>/dev/null); echo "  set_ec node: ${SE:-<none>}  (write-test requires lockdown=integrity boot)"

echo
echo "=== SUMMARY: $pass passed, $fail failed (EC-objective checks) ==="
