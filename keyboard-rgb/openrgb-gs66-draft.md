# OpenRGB GS66 (SteelSeries KLC, 1038:113a) — contribution draft

Target: `Controllers/MSILaptopController/` on branch `msi-gs66-klc`.
Data source: authoritative, from MSI Center `MysticLight_AllDevice.dll` v1.0.0.96
(`GE73Keys` + `Group1..6_Offset`), extracted to `gs66-keymap.json`. Validated on hardware.

## 1. DONE (applied to working tree): count-truncation fix
`MSILaptopController.cpp` — `buf[3]` is the LED entry count, not a fixed packet id. The old
hard-coded `0x66` (=102) truncated keyboards >102 LEDs (GS66 has 106). Changed KLC path to
`buf[3] = (unsigned char)led_count`. Verified on-hardware (0x66 left last keys unset; led_count
set all). Independently valid — improves the existing Raider A18 KLC too.

## 2. Detector (add to MSILaptopControllerDetect.cpp)
```c
#define STEELSERIES_MSI_GS66_KLC_PID   0x113A
...
REGISTER_HID_DETECTOR("MSI GS66 Keyboard", DetectMSILaptop, STEELSERIES_VID, STEELSERIES_MSI_GS66_KLC_PID);
```
`DetectMSILaptop` already maps any non-ALC PID to `MSI_LAPTOP_KLC`, so only the model lookup +
detector line are needed.

## 3. Model entry LED table (88 keys, KLC only — GS66 15" has no numpad, no lightbar)
`sys_vendor = "Micro-Star International Co., Ltd."`, `product_name = "Stealth GS66 12UHS"` (board MS-16V5).
```c
static const msi_laptop_led msi_stealth_gs66_klc_leds[] =
{
    { KEY_EN_A,                    0x04 },   // CLK_A  (Group1_Offset)
    { KEY_EN_B,                    0x05 },   // CLK_B  (Group3_Offset)
    { KEY_EN_C,                    0x06 },   // CLK_C  (Group2_Offset)
    { KEY_EN_D,                    0x07 },   // CLK_D  (Group2_Offset)
    { KEY_EN_E,                    0x08 },   // CLK_E  (Group2_Offset)
    { KEY_EN_F,                    0x09 },   // CLK_F  (Group2_Offset)
    { KEY_EN_G,                    0x0A },   // CLK_G  (Group2_Offset)
    { KEY_EN_H,                    0x0B },   // CLK_H  (Group3_Offset)
    { KEY_EN_I,                    0x0C },   // CLK_I  (Group3_Offset)
    { KEY_EN_J,                    0x0D },   // CLK_J  (Group3_Offset)
    { KEY_EN_K,                    0x0E },   // CLK_K  (Group3_Offset)
    { KEY_EN_L,                    0x0F },   // CLK_L  (Group4_Offset)
    { KEY_EN_M,                    0x10 },   // CLK_M  (Group3_Offset)
    { KEY_EN_N,                    0x11 },   // CLK_N  (Group3_Offset)
    { KEY_EN_O,                    0x12 },   // CLK_O  (Group4_Offset)
    { KEY_EN_P,                    0x13 },   // CLK_P  (Group4_Offset)
    { KEY_EN_Q,                    0x14 },   // CLK_Q  (Group1_Offset)
    { KEY_EN_R,                    0x15 },   // CLK_R  (Group2_Offset)
    { KEY_EN_S,                    0x16 },   // CLK_S  (Group1_Offset)
    { KEY_EN_T,                    0x17 },   // CLK_T  (Group2_Offset)
    { KEY_EN_U,                    0x18 },   // CLK_U  (Group3_Offset)
    { KEY_EN_V,                    0x19 },   // CLK_V  (Group2_Offset)
    { KEY_EN_W,                    0x1A },   // CLK_W  (Group1_Offset)
    { KEY_EN_X,                    0x1B },   // CLK_X  (Group2_Offset)
    { KEY_EN_Y,                    0x1C },   // CLK_Y  (Group3_Offset)
    { KEY_EN_Z,                    0x1D },   // CLK_Z  (Group1_Offset)
    { KEY_EN_1,                    0x1E },   // CLK_1  (Group1_Offset)
    { KEY_EN_2,                    0x1F },   // CLK_2  (Group1_Offset)
    { KEY_EN_3,                    0x20 },   // CLK_3  (Group2_Offset)
    { KEY_EN_4,                    0x21 },   // CLK_4  (Group2_Offset)
    { KEY_EN_5,                    0x22 },   // CLK_5  (Group2_Offset)
    { KEY_EN_6,                    0x23 },   // CLK_6  (Group3_Offset)
    { KEY_EN_7,                    0x24 },   // CLK_7  (Group3_Offset)
    { KEY_EN_8,                    0x25 },   // CLK_8  (Group3_Offset)
    { KEY_EN_9,                    0x26 },   // CLK_9  (Group4_Offset)
    { KEY_EN_0,                    0x27 },   // CLK_0  (Group4_Offset)
    { KEY_EN_ANSI_ENTER,           0x28 },   // CLK_Enter  (Group5_Offset)
    { KEY_EN_ESCAPE,               0x29 },   // CLK_Escape  (Group1_Offset)
    { KEY_EN_BACKSPACE,            0x2A },   // CLK_Backspace  (Group5_Offset)
    { KEY_EN_TAB,                  0x2B },   // CLK_Tab  (Group1_Offset)
    { KEY_EN_SPACE,                0x2C },   // CLK_Space  (Group3_Offset)
    { KEY_EN_MINUS,                0x2D },   // CLK_MinusAndUnderscore  (Group4_Offset)
    { KEY_EN_EQUALS,               0x2E },   // CLK_EqualsAndPlus  (Group5_Offset)
    { KEY_EN_LEFT_BRACKET,         0x2F },   // CLK_BracketLeft  (Group4_Offset)
    { KEY_EN_RIGHT_BRACKET,        0x30 },   // CLK_BracketRight  (Group5_Offset)
    { KEY_EN_BACK_SLASH,           0x31 },   // CLK_Backslash  (Group5_Offset)
    { KEY_EN_SEMICOLON,            0x33 },   // CLK_SemicolonAndColon  (Group4_Offset)
    { KEY_EN_QUOTE,                0x34 },   // CLK_ApostropheAndDoubleQuote  (Group4_Offset)
    { KEY_EN_BACK_TICK,            0x35 },   // CLK_GraveAccentAndTilde  (Group1_Offset)
    { KEY_EN_COMMA,                0x36 },   // CLK_CommaAndLessThan  (Group4_Offset)
    { KEY_EN_PERIOD,               0x37 },   // CLK_PeriodAndBiggerThan  (Group4_Offset)
    { KEY_EN_FORWARD_SLASH,        0x38 },   // CLK_SlashAndQuestionMark  (Group4_Offset)
    { KEY_EN_CAPS_LOCK,            0x39 },   // CLK_CapsLock  (Group1_Offset)
    { KEY_EN_F1,                   0x3A },   // CLK_F1  (Group1_Offset)
    { KEY_EN_F2,                   0x3B },   // CLK_F2  (Group1_Offset)
    { KEY_EN_F3,                   0x3C },   // CLK_F3  (Group2_Offset)
    { KEY_EN_F4,                   0x3D },   // CLK_F4  (Group2_Offset)
    { KEY_EN_F5,                   0x3E },   // CLK_F5  (Group2_Offset)
    { KEY_EN_F6,                   0x3F },   // CLK_F6  (Group3_Offset)
    { KEY_EN_F7,                   0x40 },   // CLK_F7  (Group3_Offset)
    { KEY_EN_F8,                   0x41 },   // CLK_F8  (Group3_Offset)
    { KEY_EN_F9,                   0x42 },   // CLK_F9  (Group3_Offset)
    { KEY_EN_F10,                  0x43 },   // CLK_F10  (Group4_Offset)
    { KEY_EN_F11,                  0x44 },   // CLK_F11  (Group4_Offset)
    { KEY_EN_F12,                  0x45 },   // CLK_F12  (Group4_Offset)
    { KEY_EN_PRINT_SCREEN,         0x46 },   // CLK_PrintScreen  (Group5_Offset)
    { KEY_EN_SCROLL_LOCK,          0x47 },   // CLK_ScrollLock  (Group5_Offset)
    { KEY_EN_PAUSE_BREAK,          0x48 },   // CLK_PauseBreak  (Group5_Offset)
    { KEY_EN_INSERT,               0x49 },   // CLK_Insert  (Group6_Offset)
    { KEY_EN_HOME,                 0x4A },   // CLK_Home  (Group1_Offset)
    { "Home/Page Up",              0x4B },   // CLK_PageUpAndHome  (Group6_Offset)
    { KEY_EN_DELETE,               0x4C },   // CLK_Delete  (Group6_Offset)
    { KEY_EN_END,                  0x4D },   // CLK_End  (Group1_Offset)
    { KEY_EN_PAGE_DOWN,            0x4E },   // CLK_PageDownAndEnd  (Group6_Offset)
    { KEY_EN_RIGHT_ARROW,          0x4F },   // CLK_RightArrow  (Group6_Offset)
    { KEY_EN_LEFT_ARROW,           0x50 },   // CLK_LeftArrow  (Group5_Offset)
    { KEY_EN_DOWN_ARROW,           0x51 },   // CLK_DownArrow  (Group5_Offset)
    { KEY_EN_UP_ARROW,             0x52 },   // CLK_UpArrow  (Group5_Offset)
    { KEY_EN_ISO_BACK_SLASH,       0x64 },   // CLK_Backslash2  (Group4_Offset)
    { KEY_EN_POWER,                0x66 },   // CLK_Power  (Group6_Offset)
    { KEY_EN_LEFT_CONTROL,         0xE0 },   // CLK_LeftCtrl  (Group1_Offset)
    { KEY_EN_LEFT_SHIFT,           0xE1 },   // CLK_LeftShift  (Group1_Offset)
    { KEY_EN_LEFT_ALT,             0xE2 },   // CLK_LeftAlt  (Group2_Offset)
    { KEY_EN_LEFT_WINDOWS,         0xE3 },   // CLK_LeftGui  (Group4_Offset)
    { KEY_EN_RIGHT_WINDOWS,        0xE4 },   // CLK_RightCtrl  (Group5_Offset)
    { KEY_EN_RIGHT_SHIFT,          0xE5 },   // CLK_RightShift  (Group5_Offset)
    { KEY_EN_RIGHT_ALT,            0xE6 },   // CLK_RightAlt  (Group4_Offset)
    { KEY_EN_RIGHT_FUNCTION,       0xF0 },   // CLK_Fn  (Group1_Offset)
};
```

## 4. Matrix map — NEEDS ON-HARDWARE VERIFICATION (do not guess)
The `klc_matrix_map` (physical row/col grid → led index) drives OpenRGB's keyboard view. Unlike the
LED codes (authoritative), the physical grid must be verified per unit. Build it by lighting one key
at a time and noting its position:
```sh
sudo ./msi-nb-rgb.py off
sudo ./msi-nb-rgb.py keys CLK_Escape=ff0000     # note which physical key lights; repeat per key
```
GS66 is a 6-row, ~15-col ANSI layout (no numpad). Until verified, ship LED control (direct mode)
without a matrix, or reuse the Raider grid minus numpad columns and mark experimental.

## NOTE on nav-cluster HID codes to verify
GE73Keys defines BOTH `CLK_Home`(0x4A)+`CLK_PageUpAndHome`(0x4B) and `CLK_End`(0x4D)+
`CLK_PageDownAndEnd`(0x4E). Confirm which HID each physical GS66 nav key actually lights before
finalizing names.

