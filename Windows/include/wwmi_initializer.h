#pragma once

#include "game_initializer.h"

namespace bridge {

class WWMIInitializer : public GameInitializer {
public:
    void initialize(const BridgeConfig& config, StatusReporter& reporter) override;

private:
    void configure_settings(const BridgeConfig& config, StatusReporter& reporter);
    void update_engine_ini(const BridgeConfig& config, StatusReporter& reporter);
    void update_game_user_settings_ini(const BridgeConfig& config, StatusReporter& reporter);
    void update_device_profiles_ini(const BridgeConfig& config, StatusReporter& reporter);
};

} // namespace bridge
