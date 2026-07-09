/*---------------------------------------------------------*\
| OpenRGBMSIKLCPlugin.h                                      |
|                                                           |
|   OpenRGB plugin adding device support for the MSI GS66    |
|   SteelSeries KLC per-key RGB keyboard (USB 1038:113A).    |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#pragma once

#include <vector>
#include <QObject>
#include <QString>
#include "OpenRGBPluginInterface.h"
#include "ResourceManagerInterface.h"
#include "RGBController_MSIKLC.h"

class OpenRGBMSIKLCPlugin : public QObject, public OpenRGBPluginInterface
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID OpenRGBPluginInterface_IID)
    Q_INTERFACES(OpenRGBPluginInterface)

public:
    OpenRGBMSIKLCPlugin();
    ~OpenRGBMSIKLCPlugin();

    /*-----------------------------------------------------*\
    | OpenRGBPluginInterface implementation                 |
    \*-----------------------------------------------------*/
    OpenRGBPluginInfo   GetPluginInfo() override;
    unsigned int        GetPluginAPIVersion() override;

    void                Load(ResourceManagerInterface* resource_manager_ptr) override;
    QWidget*            GetWidget() override;
    QMenu*              GetTrayMenu() override;
    void                Unload() override;

private:
    void                Enumerate();
    void                Reconcile();

    static void         DetectionEndFunction(void* this_ptr);

    /*-----------------------------------------------------*\
    | resource_manager: not owned (borrowed from OpenRGB).  |
    | controllers: RAW pointers on purpose - ownership is    |
    | SHARED with OpenRGB, which deletes them on a rescan.   |
    | Reconcile() drops any it already freed so we never     |
    | double-free; a unique_ptr here would be incorrect.     |
    \*-----------------------------------------------------*/
    ResourceManagerInterface*           resource_manager = nullptr;
    std::vector<RGBController_MSIKLC*>   controllers;
};
