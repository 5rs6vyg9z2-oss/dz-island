#include "../src/drivers/attack_shark_battery.h"
#include <cassert>
#include <iostream>
int main() {
    assert(AttackSharkHeartbeatPercent(0x08, 1, 73) == 73);
    assert(AttackSharkHeartbeatPercent(0x08, 0, 0) == 0);
    assert(AttackSharkHeartbeatPercent(0x08, 3, 54) == 54);
    assert(AttackSharkHeartbeatPercent(0x08, 2, 99) == 99);
    assert(AttackSharkHeartbeatPercent(0x08, 1, 101) == -1);
    assert(AttackSharkHeartbeatPercent(0x08, 0xFF, 50) == -1);
    assert(AttackSharkHeartbeatPercent(0x10, 1, 7) == -1); // Tenths-scale family A unchanged.
    assert(AttackSharkHeartbeatPercent(0x00, 3, 7) == -1); // ARDOR uses a different decoder.
    assert(AttackSharkHeartbeatPercent(0x73, 1, 70) == -1); // Inphic unchanged.
    std::cout << "Attack Shark heartbeat: passed\n";
}
