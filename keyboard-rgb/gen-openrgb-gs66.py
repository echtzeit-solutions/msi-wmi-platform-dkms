#!/usr/bin/env python3
"""Generate OpenRGB C arrays for the GS66 (SteelSeries KLC, 1038:113a) from gs66-keymap.json.

Emits:
  - msi_gs66_klc_leds[]        : {KEY_EN_* name, hid}  (per-key control; HID codes are authoritative)
  - msi_gs66_klc_matrix_map[H][W] : physical grid of indices into leds[] (NA = gap)

The HID usage codes in gs66-keymap.json are identical to OpenRGB's existing msi_raider_a18_klc_leds
table, so each key maps 1:1 onto a KEY_EN_* constant (RGBControllerKeyNames.h).

NOTE: leds[] (name+hid) is hardware-authoritative. The matrix_map PHYSICAL positions are a best-effort
US-ANSI GS66 layout and should be visually verified in OpenRGB against the real keyboard (this only
affects the on-screen grid, never per-key control correctness).
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
KM = json.load(open(os.path.join(HERE, "gs66-keymap.json")))["keys"]

# CLK_* (MSI name) -> OpenRGB KEY_EN_* constant (from RGBControllerKeyNames.h / msi_raider_a18_klc_leds)
NAME = {
    "CLK_A":"KEY_EN_A","CLK_B":"KEY_EN_B","CLK_C":"KEY_EN_C","CLK_D":"KEY_EN_D","CLK_E":"KEY_EN_E",
    "CLK_F":"KEY_EN_F","CLK_G":"KEY_EN_G","CLK_H":"KEY_EN_H","CLK_I":"KEY_EN_I","CLK_J":"KEY_EN_J",
    "CLK_K":"KEY_EN_K","CLK_L":"KEY_EN_L","CLK_M":"KEY_EN_M","CLK_N":"KEY_EN_N","CLK_O":"KEY_EN_O",
    "CLK_P":"KEY_EN_P","CLK_Q":"KEY_EN_Q","CLK_R":"KEY_EN_R","CLK_S":"KEY_EN_S","CLK_T":"KEY_EN_T",
    "CLK_U":"KEY_EN_U","CLK_V":"KEY_EN_V","CLK_W":"KEY_EN_W","CLK_X":"KEY_EN_X","CLK_Y":"KEY_EN_Y","CLK_Z":"KEY_EN_Z",
    "CLK_1":"KEY_EN_1","CLK_2":"KEY_EN_2","CLK_3":"KEY_EN_3","CLK_4":"KEY_EN_4","CLK_5":"KEY_EN_5",
    "CLK_6":"KEY_EN_6","CLK_7":"KEY_EN_7","CLK_8":"KEY_EN_8","CLK_9":"KEY_EN_9","CLK_0":"KEY_EN_0",
    "CLK_Enter":"KEY_EN_ANSI_ENTER","CLK_Escape":"KEY_EN_ESCAPE","CLK_Backspace":"KEY_EN_BACKSPACE",
    "CLK_Tab":"KEY_EN_TAB","CLK_Space":"KEY_EN_SPACE","CLK_MinusAndUnderscore":"KEY_EN_MINUS",
    "CLK_EqualsAndPlus":"KEY_EN_EQUALS","CLK_BracketLeft":"KEY_EN_LEFT_BRACKET",
    "CLK_BracketRight":"KEY_EN_RIGHT_BRACKET","CLK_Backslash":"KEY_EN_BACK_SLASH",
    "CLK_SemicolonAndColon":"KEY_EN_SEMICOLON","CLK_ApostropheAndDoubleQuote":"KEY_EN_QUOTE",
    "CLK_GraveAccentAndTilde":"KEY_EN_BACK_TICK","CLK_CommaAndLessThan":"KEY_EN_COMMA",
    "CLK_PeriodAndBiggerThan":"KEY_EN_PERIOD","CLK_SlashAndQuestionMark":"KEY_EN_FORWARD_SLASH",
    "CLK_CapsLock":"KEY_EN_CAPS_LOCK","CLK_F1":"KEY_EN_F1","CLK_F2":"KEY_EN_F2","CLK_F3":"KEY_EN_F3",
    "CLK_F4":"KEY_EN_F4","CLK_F5":"KEY_EN_F5","CLK_F6":"KEY_EN_F6","CLK_F7":"KEY_EN_F7","CLK_F8":"KEY_EN_F8",
    "CLK_F9":"KEY_EN_F9","CLK_F10":"KEY_EN_F10","CLK_F11":"KEY_EN_F11","CLK_F12":"KEY_EN_F12",
    "CLK_PrintScreen":"KEY_EN_PRINT_SCREEN","CLK_ScrollLock":"KEY_EN_SCROLL_LOCK",
    "CLK_PauseBreak":"KEY_EN_PAUSE_BREAK","CLK_Insert":"KEY_EN_INSERT","CLK_Home":"KEY_EN_HOME",
    "CLK_PageUpAndHome":"KEY_EN_PAGE_UP","CLK_Delete":"KEY_EN_DELETE","CLK_End":"KEY_EN_END",
    "CLK_PageDownAndEnd":"KEY_EN_PAGE_DOWN","CLK_RightArrow":"KEY_EN_RIGHT_ARROW",
    "CLK_LeftArrow":"KEY_EN_LEFT_ARROW","CLK_DownArrow":"KEY_EN_DOWN_ARROW","CLK_UpArrow":"KEY_EN_UP_ARROW",
    "CLK_Backslash2":"KEY_EN_ISO_BACK_SLASH","CLK_Power":"KEY_EN_POWER","CLK_LeftCtrl":"KEY_EN_LEFT_CONTROL",
    "CLK_LeftShift":"KEY_EN_LEFT_SHIFT","CLK_LeftAlt":"KEY_EN_LEFT_ALT","CLK_LeftGui":"KEY_EN_LEFT_WINDOWS",
    "CLK_RightCtrl":"KEY_EN_RIGHT_CONTROL","CLK_RightShift":"KEY_EN_RIGHT_SHIFT",
    "CLK_RightAlt":"KEY_EN_RIGHT_ALT","CLK_Fn":"KEY_EN_RIGHT_FUNCTION",
}

# Best-effort US-ANSI GS66 physical layout, row-major. None = gap in the grid.
# VERIFY against the real keyboard in OpenRGB; affects only the on-screen grid.
LAYOUT = [
    ["CLK_Escape","CLK_F1","CLK_F2","CLK_F3","CLK_F4","CLK_F5","CLK_F6","CLK_F7","CLK_F8","CLK_F9","CLK_F10","CLK_F11","CLK_F12","CLK_Delete","CLK_Power"],
    ["CLK_GraveAccentAndTilde","CLK_1","CLK_2","CLK_3","CLK_4","CLK_5","CLK_6","CLK_7","CLK_8","CLK_9","CLK_0","CLK_MinusAndUnderscore","CLK_EqualsAndPlus","CLK_Backspace","CLK_Home"],
    ["CLK_Tab","CLK_Q","CLK_W","CLK_E","CLK_R","CLK_T","CLK_Y","CLK_U","CLK_I","CLK_O","CLK_P","CLK_BracketLeft","CLK_BracketRight","CLK_Backslash","CLK_End"],
    ["CLK_CapsLock","CLK_A","CLK_S","CLK_D","CLK_F","CLK_G","CLK_H","CLK_J","CLK_K","CLK_L","CLK_SemicolonAndColon","CLK_ApostropheAndDoubleQuote","CLK_Enter",None,"CLK_PageUpAndHome"],
    ["CLK_LeftShift","CLK_Backslash2","CLK_Z","CLK_X","CLK_C","CLK_V","CLK_B","CLK_N","CLK_M","CLK_CommaAndLessThan","CLK_PeriodAndBiggerThan","CLK_SlashAndQuestionMark","CLK_RightShift","CLK_UpArrow","CLK_PageDownAndEnd"],
    ["CLK_LeftCtrl","CLK_Fn","CLK_LeftGui","CLK_LeftAlt","CLK_Space","CLK_RightAlt","CLK_RightCtrl","CLK_Insert","CLK_PrintScreen","CLK_ScrollLock","CLK_PauseBreak","CLK_LeftArrow","CLK_DownArrow","CLK_RightArrow",None],
]

def main():
    # leds[] in row-major layout order; index = position in this list
    order = [k for row in LAYOUT for k in row if k]
    seen = set(order)
    # any key in the keymap not placed in LAYOUT still gets an led entry (no grid cell)
    extras = [k for k in KM if k not in seen]
    leds = order + extras
    idx = {k: i for i, k in enumerate(leds)}

    W = max(len(r) for r in LAYOUT)
    H = len(LAYOUT)

    out = []
    out.append(f"/* GS66 KLC per-key layout - generated from gs66-keymap.json ({len(leds)} keys). */")
    out.append("static const msi_laptop_led msi_gs66_klc_leds[] =\n{")
    for k in leds:
        out.append(f'    {{ {NAME[k]:<26}, 0x{KM[k]["hid"]:02X} }},   // {k}')
    out.append("};\n")
    if extras:
        out.append(f"/* keys present in keymap but not placed in the physical grid: {', '.join(extras)} */")
    out.append(f"#define MSI_GS66_KLC_MATRIX_HEIGHT  {H}")
    out.append(f"#define MSI_GS66_KLC_MATRIX_WIDTH   {W}")
    out.append("/* NA = gap. VERIFY physical positions in OpenRGB against the real keyboard. */")
    out.append(f"static unsigned int msi_gs66_klc_matrix_map[MSI_GS66_KLC_MATRIX_HEIGHT][MSI_GS66_KLC_MATRIX_WIDTH] =\n{{")
    for row in LAYOUT:
        cells = []
        for c in range(W):
            k = row[c] if c < len(row) else None
            cells.append("  NA" if k is None else f"{idx[k]:4d}")
        out.append("    { " + ", ".join(cells) + " },")
    out.append("};")

    txt = "\n".join(out)
    dst = os.path.join(HERE, "openrgb-gs66-arrays.h")
    open(dst, "w").write(txt + "\n")
    print(f"{len(leds)} leds, matrix {H}x{W} -> {dst}")
    missing = [k for k in KM if k not in NAME]
    if missing:
        print("WARNING unmapped CLK names:", missing, file=sys.stderr)

if __name__ == "__main__":
    main()
