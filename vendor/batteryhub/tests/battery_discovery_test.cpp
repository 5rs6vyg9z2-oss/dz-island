#include "../src/battery_discovery.h"
#include <cassert>
#include <iostream>

int main() {
    using namespace battery_discovery;
    DeviceStatus vendor;
    vendor.vid = 0x1D57; vendor.pid = 0xFA60; vendor.key = L"vendor-device";
    DeviceStatus standard; standard.percent = 72; standard.source = L"standard-hid";
    std::vector<DeviceStatus> devices{vendor};
    assert(MergeFallback(devices, standard, vendor.vid, vendor.pid));
    assert(devices.size() == 1 && devices[0].percent == 72 && devices[0].key == vendor.key);
    standard.percent = 12;
    assert(MergeFallback(devices, standard, vendor.vid, vendor.pid));
    assert(devices[0].percent == 72);  // Preserve an existing usable reading.
    assert(!MergeFallback(devices, standard, vendor.vid, 0xFA65));
    assert(!MergeFallback(devices, standard, 0, 0));
    devices = {vendor, vendor};
    assert(MergeFallback(devices, standard, vendor.vid, vendor.pid));
    assert(devices[0].percent == -1 && devices[1].percent == -1); // No cross-device attribution.
    devices = {vendor}; devices[0].asleep = true;
    MergeFallback(devices, standard, vendor.vid, vendor.pid);
    assert(devices[0].percent == -1);
    devices[0].asleep = false; devices[0].connected = false;
    MergeFallback(devices, standard, vendor.vid, vendor.pid);
    assert(devices[0].percent == -1);
    devices = {vendor}; standard.percent = 255;
    MergeFallback(devices, standard, vendor.vid, vendor.pid);
    assert(devices[0].percent == -1);
    devices[0].levelText = L"Low";
    assert(!NeedsFallback(devices[0]));
    assert(WirelessName(L"ATTACK SHARK Wireless Receiver"));
    assert(WirelessName(L"2.4G Dongle"));
    assert(!WirelessName(L"USB Wired Keyboard"));
    assert(ReceiverCandidate(0x1D57, 0xFA60, L"USB Gaming Mouse"));
    assert(ReceiverCandidate(0x1D57, 0xFA65, L""));
    assert(!ReceiverCandidate(0x1D57, 0x2126, L"USB Mouse"));
    std::cout << "battery discovery policy: passed\n";
}
