#pragma once
// Read-only heartbeat decoding for known percent-scale BK family-B models.
// Protocol/model references: lucasbrito-wdt/attack-shark-configurator,
// docs/protocol/family-b.md section 4.2 and platform/bk/registry.rs (GPL-3.0).
// No commands, configuration writes, or guessed decoding for unknown model codes.
#include <cstdint>

inline bool AttackSharkPercentModel(std::uint8_t code) {
    switch (code) {
        case 0xBE: case 0x01: case 0x03: case 0x07: case 0x02: case 0x06:
        case 0x08: case 0x04: case 0x0E: case 0xEA: case 0xB1: case 0x4E:
        case 0xE5: case 0x24: return true;
        default: return false;
    }
}
inline int AttackSharkHeartbeatPercent(std::uint8_t code, std::uint8_t state, std::uint8_t level) {
    if (!AttackSharkPercentModel(code) || state > 3 || level > 100) return -1;
    return level; // Receiver reports a percentage even while charging; do not invent 100%.
}
