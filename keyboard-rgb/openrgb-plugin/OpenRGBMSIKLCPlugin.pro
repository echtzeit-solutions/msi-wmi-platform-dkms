#-----------------------------------------------------------------------------#
# OpenRGBMSIKLCPlugin.pro                                                       #
#                                                                              #
#   Out-of-tree OpenRGB plugin: MSI GS66 SteelSeries KLC per-key RGB keyboard  #
#   (USB 1038:113A).                                                           #
#                                                                              #
#   Build (Qt6):  qmake6 && make                                               #
#   Build (Qt5):  /usr/lib/qt5/bin/qmake && make    (or plain `qmake`)         #
#     The source is Qt5- and Qt6-compatible; qmake picks up whichever Qt its   #
#     binary belongs to. Point OPENRGB_ROOT at the matching OpenRGB source     #
#     (e.g. `apt source openrgb` for the distro package).                      #
#   Install: copy the resulting .so into ~/.config/OpenRGB/plugins/            #
#                                                                              #
#   The plugin ABI is an EXACT-MATCH against OPENRGB_PLUGIN_API_VERSION and    #
#   the .so is NOT ABI-portable: rebuild against the same OpenRGB source tree  #
#   and the same Qt MAJOR version (5 or 6) your OpenRGB binary uses.           #
#-----------------------------------------------------------------------------#

#-----------------------------------------------------------------------------#
# Path to the OpenRGB source tree this plugin is built against (read-only).    #
# Override on the command line with: qmake6 OPENRGB_ROOT=/path/to/OpenRGB      #
#-----------------------------------------------------------------------------#
isEmpty(OPENRGB_ROOT) {
    OPENRGB_ROOT = /home/florian/src-laptop/OpenRGB
}

TEMPLATE  = lib
CONFIG   += plugin c++17
QT       += core gui widgets

TARGET    = OpenRGBMSIKLCPlugin

DEFINES  += ENTRY_EXPORT

#-----------------------------------------------------------------------------#
# Include paths into the OpenRGB source tree.                                  #
#-----------------------------------------------------------------------------#
INCLUDEPATH += \
    $$PWD                                           \
    $$OPENRGB_ROOT                                  \
    $$OPENRGB_ROOT/RGBController                    \
    $$OPENRGB_ROOT/i2c_smbus

#-----------------------------------------------------------------------------#
# Plugin sources.                                                              #
#-----------------------------------------------------------------------------#
HEADERS += \
    $$PWD/OpenRGBMSIKLCPlugin.h                     \
    $$PWD/RGBController_MSIKLC.h                    \
    $$PWD/MSIKLCController.h                        \
    $$PWD/openrgb-gs66-arrays.h

SOURCES += \
    $$PWD/OpenRGBMSIKLCPlugin.cpp                   \
    $$PWD/RGBController_MSIKLC.cpp                  \
    $$PWD/MSIKLCController.cpp

#-----------------------------------------------------------------------------#
# OpenRGB sources compiled into the plugin (base class + key-name strings).    #
#-----------------------------------------------------------------------------#
SOURCES += \
    $$OPENRGB_ROOT/RGBController/RGBController.cpp  \
    $$OPENRGB_ROOT/RGBController/RGBControllerKeyNames.cpp

#-----------------------------------------------------------------------------#
# hidapi header discovery.                                                     #
#   Full OpenRGB tree vendors it under dependencies/; the distro (apt source)  #
#   package strips it (uses external libhidapi). Prefer the vendored copy,     #
#   else rely on pkg-config cflags / the system include dir below.             #
#-----------------------------------------------------------------------------#
exists($$OPENRGB_ROOT/dependencies/hidapi-win/include/hidapi.h) {
    INCLUDEPATH += $$OPENRGB_ROOT/dependencies/hidapi-win/include
}

#-----------------------------------------------------------------------------#
# hidapi linkage (+ system headers when using the distro libhidapi).           #
#   Prefer pkg-config (hidapi-hidraw / -libusb) when a -dev package is         #
#   installed - it also supplies the include path via cflags. Otherwise link   #
#   the runtime SONAME directly and add the common system header location.     #
#-----------------------------------------------------------------------------#
packagesExist(hidapi-hidraw) {
    CONFIG   += link_pkgconfig
    PKGCONFIG += hidapi-hidraw
} else {
    packagesExist(hidapi-libusb) {
        CONFIG   += link_pkgconfig
        PKGCONFIG += hidapi-libusb
    } else {
        LIBS += -l:libhidapi-hidraw.so.0
        exists(/usr/include/hidapi/hidapi.h): INCLUDEPATH += /usr/include/hidapi
    }
}
