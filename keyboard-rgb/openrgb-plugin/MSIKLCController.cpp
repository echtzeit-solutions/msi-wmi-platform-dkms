/*---------------------------------------------------------*\
| MSIKLCController.cpp                                       |
|                                                           |
|   Raw HID protocol driver for the MSI GS66 SteelSeries    |
|   KLC per-key RGB keyboard (USB 1038:113A).               |
|                                                           |
|   The Direct-mode wire format mirrors OpenRGB's in-tree   |
|   MSILaptopController: a 525-byte feature report,         |
|   buf[1] = 0x0C command, buf[3] = LED entry count, a      |
|   4-byte {keycode, R, G, B} record per LED starting at    |
|   offset 5, unused slots padded with 0xFF.                |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#include <cstring>
#include "MSIKLCController.h"

#define MSI_KLC_REPORT_ID       0x00
#define MSI_KLC_COMMAND         0x0C
#define MSI_KLC_PACKET_SIZE     525
#define MSI_KLC_PAYLOAD_OFFSET  5

MSIKLCController::MSIKLCController(hid_device* dev_handle, const char* path, std::string dev_name)
    : dev(dev_handle),
      location((path != nullptr) ? std::string(path) : std::string()),
      name(std::move(dev_name))
{
    /*-----------------------------------------------------*\
    | color_scale / brightness use in-class initializers.   |
    \*-----------------------------------------------------*/
}

MSIKLCController::~MSIKLCController()
{
    /*-----------------------------------------------------*\
    | dev is a std::unique_ptr<hid_device, HidDeviceDeleter>|
    | so the handle is hid_close()d automatically here.     |
    \*-----------------------------------------------------*/
}

std::string MSIKLCController::GetDeviceLocation()
{
    return("HID: " + location);
}

std::string MSIKLCController::GetDeviceName()
{
    return(name);
}

std::string MSIKLCController::GetSerialString()
{
    if(dev == nullptr)
    {
        return("");
    }

    wchar_t serial_string[128];
    int ret = hid_get_serial_number_string(dev.get(), serial_string, 128);

    if(ret != 0)
    {
        return("");
    }

    /*-----------------------------------------------------*\
    | Minimal wide-to-narrow conversion (serials are ASCII) |
    | to avoid pulling in OpenRGB's StringUtils.cpp.        |
    \*-----------------------------------------------------*/
    std::string result;

    for(unsigned int i = 0; (i < 128) && (serial_string[i] != L'\0'); i++)
    {
        result += (char)serial_string[i];
    }

    return(result);
}

void MSIKLCController::SetColorScale(float r, float g, float b)
{
    color_scale[0] = r;
    color_scale[1] = g;
    color_scale[2] = b;
}

void MSIKLCController::SetBrightness(unsigned char value)
{
    brightness = value;
}

void MSIKLCController::apply_correction(unsigned char& r, unsigned char& g, unsigned char& b, const float scale[3], unsigned char brightness_value)
{
    /*-----------------------------------------------------*\
    | effective[i] = color_scale[i] * (brightness / 255).   |
    | Brightness is software-owned on this firmware build   |
    | (no host HID brightness path); see KLC-PROTOCOL.md.   |
    \*-----------------------------------------------------*/
    float bright_factor = (float)brightness_value / 255.0f;

    int red   = (int)((float)r * scale[0] * bright_factor + 0.5f);
    int green = (int)((float)g * scale[1] * bright_factor + 0.5f);
    int blue  = (int)((float)b * scale[2] * bright_factor + 0.5f);

    if(red   > 255) { red   = 255; }
    if(green > 255) { green = 255; }
    if(blue  > 255) { blue  = 255; }
    if(red   < 0)   { red   = 0;   }
    if(green < 0)   { green = 0;   }
    if(blue  < 0)   { blue  = 0;   }

    r = (unsigned char)red;
    g = (unsigned char)green;
    b = (unsigned char)blue;
}

void MSIKLCController::SetLEDs(std::vector<led>& leds, std::vector<RGBColor>& colors)
{
    if(dev == nullptr)
    {
        return;
    }

    unsigned char   buf[MSI_KLC_PACKET_SIZE];
    std::size_t     led_count = (leds.size() < colors.size()) ? leds.size() : colors.size();

    memset(buf, 0x00, sizeof(buf));

    buf[0x00] = MSI_KLC_REPORT_ID;
    buf[0x01] = MSI_KLC_COMMAND;

    /*-----------------------------------------------------*\
    | buf[3] is the number of LED entries the controller    |
    | reads (MSI Center's own driver sets it to the entry   |
    | count, not a fixed packet id).                        |
    \*-----------------------------------------------------*/
    buf[0x03] = (unsigned char)led_count;

    /*-----------------------------------------------------*\
    | Fill unused LED IDs with 0xFF so they are ignored.    |
    \*-----------------------------------------------------*/
    for(int i = 0; i < (MSI_KLC_PACKET_SIZE - MSI_KLC_PAYLOAD_OFFSET) / 4; i++)
    {
        buf[MSI_KLC_PAYLOAD_OFFSET + (i * 4)] = 0xFF;
    }

    for(std::size_t led_idx = 0; led_idx < led_count; led_idx++)
    {
        std::size_t offset = MSI_KLC_PAYLOAD_OFFSET + (led_idx * 4);

        if((offset + 3) >= sizeof(buf))
        {
            break;
        }

        unsigned char red   = RGBGetRValue(colors[led_idx]);
        unsigned char green = RGBGetGValue(colors[led_idx]);
        unsigned char blue  = RGBGetBValue(colors[led_idx]);

        apply_correction(red, green, blue, color_scale, brightness);

        buf[offset + 0] = (unsigned char)leds[led_idx].value;
        buf[offset + 1] = red;
        buf[offset + 2] = green;
        buf[offset + 3] = blue;
    }

    hid_send_feature_report(dev.get(), buf, sizeof(buf));
}
