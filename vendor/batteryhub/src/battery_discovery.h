#pragma once
#include "devices.h"
#include <cwctype>

namespace battery_discovery {
inline bool HasReading(const DeviceStatus& device) {
    return device.percent >= 0 && device.percent <= 100;
}
inline bool NeedsFallback(const DeviceStatus& device) {
    return device.connected && !device.asleep && !HasReading(device) && device.levelText.empty();
}
// Never associate a Windows/HID reading by display name or vendor alone.
// Ambiguous identical receivers keep their independent driver readings.
inline bool MergeFallback(std::vector<DeviceStatus>& devices, const DeviceStatus& reading,
                          unsigned short vid, unsigned short pid) {
    if (!vid || !pid) return false;
    DeviceStatus* target = nullptr;
    unsigned matches = 0;
    for (auto& device : devices) {
        if (device.vid != vid || device.pid != pid) continue;
        target = &device;
        ++matches;
    }
    if (!matches) return false;
    if (matches == 1 && NeedsFallback(*target) && HasReading(reading)) {
        target->percent = reading.percent;
        target->source = reading.source;
        target->note.clear();
    }
    return true;
}
inline bool WirelessName(std::wstring name) {
    for (auto& ch : name) ch = std::towlower(ch);
    for (const auto* word : {L"wireless", L"receiver", L"dongle", L"2.4g", L"bluetooth"})
        if (name.find(word) != std::wstring::npos) return true;
    return false;
}
inline bool ReceiverCandidate(unsigned short vid, unsigned short pid, const std::wstring& name) {
    // Known shared BK wireless receivers may advertise only a generic USB product name.
    const bool bkReceiver = vid == 0x1D57 && (pid == 0xFA60 || pid == 0xFA65);
    return bkReceiver || WirelessName(name);
}
}  // namespace battery_discovery
