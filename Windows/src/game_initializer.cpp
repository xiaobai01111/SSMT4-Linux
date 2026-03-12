#include "game_initializer.h"
#include "wwmi_initializer.h"
#include "string_utils.h"

namespace bridge {

std::unique_ptr<GameInitializer> GameInitializer::create(const std::string& importer_name) {
    std::string lower = to_lower(importer_name);
    if (lower == "wwmi") {
        return std::make_unique<WWMIInitializer>();
    }
    // Future: add ZZMI, GIMI, SRMI, HIMI initializers here
    // Return nullptr if no game-specific init is needed
    return nullptr;
}

} // namespace bridge
