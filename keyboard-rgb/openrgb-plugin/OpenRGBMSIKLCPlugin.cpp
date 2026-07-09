/*---------------------------------------------------------*\
| OpenRGBMSIKLCPlugin.cpp                                    |
|                                                           |
|   OpenRGB plugin adding device support for the MSI GS66    |
|   SteelSeries KLC per-key RGB keyboard (USB 1038:113A).    |
|                                                           |
|   Out-of-tree OpenRGB plugin (echtzeit-solutions).        |
|   SPDX-License-Identifier: GPL-2.0-or-later               |
\*---------------------------------------------------------*/

#include <cstring>
#include <QLabel>
#include <Qt>
#include <hidapi.h>
#include "OpenRGBMSIKLCPlugin.h"

#define MSI_KLC_VID             0x1038
#define MSI_KLC_PID             0x113A

/*---------------------------------------------------------*\
| The vendor control collection on the GS66 keyboard.       |
| Interface 0 is the SteelSeries per-key lighting endpoint. |
| hid_enumerate reports the TOP-LEVEL collection usage:     |
| usage_page 0xFFC0 / usage 0x0001 (verified on-device).    |
| (0xF1 is the output-report usage inside the descriptor,   |
| not what the enumeration returns.) usage/usage_page are   |
| only populated by the hidraw backend when it was built    |
| with USAGE support, so interface_number == 0 is the       |
| reliable discriminator; usage is checked opportunistically|
\*---------------------------------------------------------*/
#define MSI_KLC_INTERFACE       0
#define MSI_KLC_USAGE_PAGE      0xFFC0
#define MSI_KLC_USAGE           0x0001

OpenRGBMSIKLCPlugin::OpenRGBMSIKLCPlugin()
{
    resource_manager = nullptr;
}

OpenRGBMSIKLCPlugin::~OpenRGBMSIKLCPlugin()
{
}

OpenRGBPluginInfo OpenRGBMSIKLCPlugin::GetPluginInfo()
{
    OpenRGBPluginInfo info;

    info.Name           = "MSI KLC Keyboard";
    info.Description     = "Adds device support for the MSI GS66 SteelSeries KLC per-key RGB keyboard (USB 1038:113A).";
    info.Version         = "0.1";
    info.Commit          = "";
    info.URL             = "https://github.com/echtzeit-solutions";

    /*-----------------------------------------------------*\
    | Tab location: INFORMATION, deliberately NOT DEVICES.  |
    | This is a device-PROVIDER plugin - the keyboard shows |
    | up as its own auto-created device page in the Devices |
    | tab (via RegisterRGBController). If we ALSO placed our |
    | plugin's own tab in the Devices bar, OpenRGB 0.9's     |
    | "Apply All Devices" path (OpenRGBDialog.cpp ~1355)     |
    | qobject_cast<OpenRGBDevicePage*>()s EVERY Devices-bar  |
    | tab WITHOUT a null check and dereferences it - our     |
    | non-device tab would cast to null and segfault. Keep   |
    | the plugin's informational tab out of that bar.        |
    \*-----------------------------------------------------*/
    info.Location        = OPENRGB_PLUGIN_LOCATION_INFORMATION;
    info.Label           = "MSI KLC";
    info.TabIconString   = "";

    return(info);
}

unsigned int OpenRGBMSIKLCPlugin::GetPluginAPIVersion()
{
    return(OPENRGB_PLUGIN_API_VERSION);
}

void OpenRGBMSIKLCPlugin::Load(ResourceManagerInterface* resource_manager_ptr)
{
    /*-----------------------------------------------------*\
    | Guard against a null ResourceManager - every device   |
    | operation below dereferences it.                      |
    \*-----------------------------------------------------*/
    if(resource_manager_ptr == nullptr)
    {
        return;
    }

    resource_manager = resource_manager_ptr;

    /*-----------------------------------------------------*\
    | Enumerate now for immediate availability, and register|
    | a detection-end callback so we re-add our controller  |
    | after a user-triggered rescan (which tears down and   |
    | rebuilds the ResourceManager device list).            |
    \*-----------------------------------------------------*/
    Enumerate();

    resource_manager->RegisterDetectionEndCallback(DetectionEndFunction, this);
}

QWidget* OpenRGBMSIKLCPlugin::GetWidget()
{
    /*-----------------------------------------------------*\
    | OpenRGB parents the returned widget into a plugin tab |
    | (OpenRGBPluginContainer) and takes ownership of it.   |
    | It must NOT be null: OpenRGB 0.9 wraps the result      |
    | unconditionally and a null widget segfaults inside     |
    | QWidget::setParent. This is a device-provider plugin   |
    | with no real UI, so return a minimal placeholder that  |
    | Qt will own via its parent.                            |
    \*-----------------------------------------------------*/
    QLabel* placeholder = new QLabel("MSI GS66 KLC per-key RGB keyboard support.\n"
                                     "The keyboard appears as its own device in the Devices tab.");
    placeholder->setAlignment(Qt::AlignCenter);
    placeholder->setWordWrap(true);
    return(placeholder);
}

QMenu* OpenRGBMSIKLCPlugin::GetTrayMenu()
{
    return(nullptr);
}

void OpenRGBMSIKLCPlugin::Unload()
{
    if(resource_manager == nullptr)
    {
        return;
    }

    resource_manager->UnregisterDetectionEndCallback(DetectionEndFunction, this);

    /*-----------------------------------------------------*\
    | Drop any controllers the ResourceManager already      |
    | destroyed (e.g. during a rescan) so we only tear down |
    | the ones we still own.                                 |
    \*-----------------------------------------------------*/
    Reconcile();

    for(unsigned int i = 0; i < controllers.size(); i++)
    {
        resource_manager->UnregisterRGBController(controllers[i]);
        delete controllers[i];
    }

    controllers.clear();
}

/*---------------------------------------------------------*\
| Drop tracked controllers that are no longer present in    |
| the ResourceManager's list (it deleted them on a rescan). |
| We must not delete them ourselves in that case - the      |
| ResourceManager already owns and freed them.              |
\*---------------------------------------------------------*/
void OpenRGBMSIKLCPlugin::Reconcile()
{
    if(resource_manager == nullptr)
    {
        return;
    }

    std::vector<RGBController*>& live = resource_manager->GetRGBControllers();

    std::vector<RGBController_MSIKLC*> remaining;

    for(unsigned int i = 0; i < controllers.size(); i++)
    {
        bool still_registered = false;

        for(unsigned int j = 0; j < live.size(); j++)
        {
            if(live[j] == controllers[i])
            {
                still_registered = true;
                break;
            }
        }

        if(still_registered)
        {
            remaining.push_back(controllers[i]);
        }
    }

    controllers = remaining;
}

void OpenRGBMSIKLCPlugin::Enumerate()
{
    if(resource_manager == nullptr)
    {
        return;
    }

    /*-----------------------------------------------------*\
    | Reconcile first so a rescan doesn't leave us holding  |
    | dangling pointers, then add only devices we are not   |
    | already tracking.                                     |
    \*-----------------------------------------------------*/
    Reconcile();

    hid_device_info* enumeration = hid_enumerate(MSI_KLC_VID, MSI_KLC_PID);
    hid_device_info* current     = enumeration;

    while(current != nullptr)
    {
        bool interface_match = (current->interface_number == MSI_KLC_INTERFACE);

        /*-------------------------------------------------*\
        | If the backend populated usage info, require the  |
        | vendor lighting collection; otherwise fall back   |
        | to the interface number alone.                    |
        \*-------------------------------------------------*/
        if(current->usage_page != 0)
        {
            interface_match = (current->usage_page == MSI_KLC_USAGE_PAGE) && (current->usage == MSI_KLC_USAGE);
        }

        if(interface_match && (current->path != nullptr))
        {
            bool already_tracked = false;

            for(unsigned int i = 0; i < controllers.size(); i++)
            {
                if(controllers[i]->GetLocation() == ("HID: " + std::string(current->path)))
                {
                    already_tracked = true;
                    break;
                }
            }

            if(!already_tracked)
            {
                hid_device* dev = hid_open_path(current->path);

                if(dev != nullptr)
                {
                    MSIKLCController*       controller     = new MSIKLCController(dev, current->path, "MSI GS66 KLC Keyboard");
                    RGBController_MSIKLC*   rgb_controller = new RGBController_MSIKLC(controller);

                    resource_manager->RegisterRGBController(rgb_controller);
                    controllers.push_back(rgb_controller);
                }
            }
        }

        current = current->next;
    }

    hid_free_enumeration(enumeration);
}

void OpenRGBMSIKLCPlugin::DetectionEndFunction(void* this_ptr)
{
    OpenRGBMSIKLCPlugin* plugin = (OpenRGBMSIKLCPlugin*)this_ptr;
    plugin->Enumerate();
}
