#pragma once

#include "bridge_config.h"
#include "status_reporter.h"
#include <memory>
#include <string>

namespace bridge {

class GameInitializer {
public:
    virtual ~GameInitializer() = default;
    virtual void initialize(const BridgeConfig& config, StatusReporter& reporter) = 0;

    // Factory: creates the right initializer based on config.importer name.
    // Returns nullptr if the importer has no game-specific init needed.
    static std::unique_ptr<GameInitializer> create(const std::string& importer_name);
};

} // namespace bridge
