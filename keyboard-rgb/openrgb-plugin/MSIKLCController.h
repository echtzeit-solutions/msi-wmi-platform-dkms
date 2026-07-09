/*---------------------------------------------------------*\
| MSIKLCController.h                                         |
|                                                           |
|   Raw HID protocol driver for the MSI GS66 SteelSeries    |
|   KLC per-key RGB keyboard (USB 1038:113A).               |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#pragma once

#include <memory>
#include <string>
#include <vector>
#include <hidapi.h>
#include "RGBController.h"

/*---------------------------------------------------------*\
| RAII owner for the hidapi handle: hid_close on scope exit, |
| null-safe. Lets MSIKLCController hold the device by a      |
| std::unique_ptr instead of a hand-managed raw pointer.    |
\*---------------------------------------------------------*/
struct HidDeviceDeleter
{
    void operator()(hid_device* handle) const
    {
        if(handle != nullptr)
        {
            hid_close(handle);
        }
    }
};

using HidDevicePtr = std::unique_ptr<hid_device, HidDeviceDeleter>;

/*---------------------------------------------------------*\
| LED descriptor for the generated layout tables.           |
| Shape mirrors MSILaptopController.h's msi_laptop_led so    |
| the generated openrgb-gs66-arrays.h can be included as-is. |
\*---------------------------------------------------------*/
typedef struct
{
    const char*     name;
    unsigned char   id;

} msi_laptop_led;

class MSIKLCController
{
public:
    MSIKLCController(hid_device* dev_handle, const char* path, std::string dev_name);
    ~MSIKLCController();

    std::string                 GetDeviceLocation();
    std::string                 GetDeviceName();
    std::string                 GetSerialString();

    /*-----------------------------------------------------*\
    | Per-channel color correction + software brightness.   |
    | scale defaults to identity {1,1,1}; the GS66           |
    | (msi-klc496) scale is [1.0,1.0,1.0], so it is a no-op  |
    | on this unit, but the mechanism exists for other       |
    | panels with a non-unity color_scale.                   |
    \*-----------------------------------------------------*/
    void                        SetColorScale(float r, float g, float b);
    void                        SetBrightness(unsigned char value);

    /*-----------------------------------------------------*\
    | Direct mode: push the live per-key color frame.       |
    \*-----------------------------------------------------*/
    void                        SetLEDs(std::vector<led>& leds, std::vector<RGBColor>& colors);

private:
    void                        apply_correction(unsigned char& r, unsigned char& g, unsigned char& b, const float scale[3], unsigned char brightness);

    HidDevicePtr                dev;                        /* owned (RAII, hid_close on destroy) */
    std::string                 location;
    std::string                 name;
    float                       color_scale[3] = { 1.0f, 1.0f, 1.0f };
    unsigned char               brightness     = 255;
};
