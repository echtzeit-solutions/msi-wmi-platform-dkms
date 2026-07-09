/*---------------------------------------------------------*\
| RGBController_MSIKLC.h                                     |
|                                                           |
|   RGBController for the MSI GS66 SteelSeries KLC per-key   |
|   RGB keyboard (USB 1038:113A).                           |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#pragma once

#include <memory>
#include "RGBController.h"
#include "MSIKLCController.h"

class RGBController_MSIKLC : public RGBController
{
public:
    RGBController_MSIKLC(MSIKLCController* controller_ptr);
    ~RGBController_MSIKLC();

    void    SetupZones();

    void    ResizeZone(int zone, int new_size);

    void    DeviceUpdateLEDs();
    void    UpdateZoneLEDs(int zone);
    void    UpdateSingleLED(int led);

    void    DeviceUpdateMode();

private:
    /*-----------------------------------------------------*\
    | controller is exclusively owned by this RGBController, |
    | so a std::unique_ptr models it correctly (auto hid_close|
    | + delete on destruction). std (not QSharedPointer) as   |
    | this class is not a QObject and stays Qt-free.         |
    \*-----------------------------------------------------*/
    std::unique_ptr<MSIKLCController>   controller;
};
