/*---------------------------------------------------------*\
| RGBController_MSIKLC.cpp                                   |
|                                                           |
|   RGBController for the MSI GS66 SteelSeries KLC per-key   |
|   RGB keyboard (USB 1038:113A).                           |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#include "RGBControllerKeyNames.h"
#include "RGBController_MSIKLC.h"

/*---------------------------------------------------------*\
| NA marks a gap in the physical matrix map.                |
| openrgb-gs66-arrays.h is generated (gen-openrgb-gs66.py); |
| do not hand-edit it. It needs msi_laptop_led (defined in  |
| MSIKLCController.h), the KEY_EN_* names (above) and NA.    |
\*---------------------------------------------------------*/
#define NA                                      0xFFFFFFFF
#include "openrgb-gs66-arrays.h"

#define MSI_KLC_ARRAY_SIZE(x)   (sizeof(x) / sizeof((x)[0]))

/**------------------------------------------------------------------*\
    @name MSI GS66 SteelSeries KLC Keyboard
    @category Keyboard
    @type USB
    @save :x:
    @direct :white_check_mark:
    @effects :x:
    @comment Out-of-tree plugin device for USB 1038:113A.
\*-------------------------------------------------------------------*/

RGBController_MSIKLC::RGBController_MSIKLC(MSIKLCController* controller_ptr)
    : controller(controller_ptr)
{
    name                = controller->GetDeviceName();
    vendor              = "SteelSeries";
    description         = "MSI GS66 SteelSeries KLC RGB Keyboard";
    location            = controller->GetDeviceLocation();
    serial              = controller->GetSerialString();
    type                = DEVICE_TYPE_KEYBOARD;

    mode Direct;
    Direct.name             = "Direct";
    Direct.flags            = MODE_FLAG_HAS_PER_LED_COLOR | MODE_FLAG_HAS_BRIGHTNESS;
    Direct.color_mode       = MODE_COLORS_PER_LED;
    Direct.brightness_min   = 0;
    Direct.brightness_max   = 255;
    Direct.brightness       = 255;
    modes.push_back(Direct);

    SetupZones();
}

RGBController_MSIKLC::~RGBController_MSIKLC()
{
    for(unsigned int zone_idx = 0; zone_idx < zones.size(); zone_idx++)
    {
        if(zones[zone_idx].matrix_map != nullptr)
        {
            delete zones[zone_idx].matrix_map;
            zones[zone_idx].matrix_map = nullptr;
        }
    }

    /*-----------------------------------------------------*\
    | controller is a std::unique_ptr - released here       |
    | automatically (which hid_close()s the device).        |
    \*-----------------------------------------------------*/
}

void RGBController_MSIKLC::SetupZones()
{
    zone keyboard_zone;

    keyboard_zone.name                   = ZONE_EN_KEYBOARD;
    keyboard_zone.leds_min               = MSI_KLC_ARRAY_SIZE(msi_gs66_klc_leds);
    keyboard_zone.leds_max               = MSI_KLC_ARRAY_SIZE(msi_gs66_klc_leds);
    keyboard_zone.leds_count             = MSI_KLC_ARRAY_SIZE(msi_gs66_klc_leds);

    /*-----------------------------------------------------------------*\
    | MATRIX zone: gives OpenRGB the physical keyboard grid. matrix_map |
    | is a heap matrix_map_type owning a pointer into the static layout |
    | array; the destructor frees the struct (not the static array).   |
    | Build with -DMSI_KLC_LINEAR_VIEW to fall back to a flat strip.    |
    \*-----------------------------------------------------------------*/
#ifdef MSI_KLC_LINEAR_VIEW
    keyboard_zone.type                   = ZONE_TYPE_LINEAR;
    keyboard_zone.matrix_map             = nullptr;
#else
    keyboard_zone.type                   = ZONE_TYPE_MATRIX;
    keyboard_zone.matrix_map             = new matrix_map_type;
    keyboard_zone.matrix_map->height     = MSI_GS66_KLC_MATRIX_HEIGHT;
    keyboard_zone.matrix_map->width      = MSI_GS66_KLC_MATRIX_WIDTH;
    keyboard_zone.matrix_map->map        = (unsigned int *)&msi_gs66_klc_matrix_map[0][0];
#endif

    zones.push_back(keyboard_zone);

    for(unsigned int led_idx = 0; led_idx < MSI_KLC_ARRAY_SIZE(msi_gs66_klc_leds); led_idx++)
    {
        led new_led;
        new_led.name  = msi_gs66_klc_leds[led_idx].name;
        new_led.value = msi_gs66_klc_leds[led_idx].id;
        leds.push_back(new_led);
    }

    SetupColors();
}

void RGBController_MSIKLC::ResizeZone(int /*zone*/, int /*new_size*/)
{
}

void RGBController_MSIKLC::DeviceUpdateLEDs()
{
    controller->SetBrightness((unsigned char)modes[active_mode].brightness);
    controller->SetLEDs(leds, colors);
}

void RGBController_MSIKLC::UpdateZoneLEDs(int /*zone*/)
{
    DeviceUpdateLEDs();
}

void RGBController_MSIKLC::UpdateSingleLED(int /*led*/)
{
    DeviceUpdateLEDs();
}

void RGBController_MSIKLC::DeviceUpdateMode()
{
    DeviceUpdateLEDs();
}
