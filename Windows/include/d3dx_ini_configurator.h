#pragma once

#include "bridge_config.h"
#include "ini_handler.h"
#include <memory>

namespace bridge {

class D3dxIniConfigurator {
public:
    explicit D3dxIniConfigurator(const BridgeConfig& config);

    void update();

    IniHandler* get_ini() { return ini_.get(); }

private:
    const BridgeConfig& config_;
    std::unique_ptr<IniHandler> ini_;

    void set_target_exe();
    void set_init_delay();
    void set_screen_resolution();
    void set_proxy_d3d11();
    void apply_constant_settings(const std::string& setting_name);
    void apply_bool_settings(const std::string& setting_name, bool value);
};

} // namespace bridge
