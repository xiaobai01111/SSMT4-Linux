#pragma once

#include "bridge_config.h"
#include "signature_verifier.h"
#include "status_reporter.h"
#include <string>
#include <vector>
#include <memory>

namespace bridge {

class PackageDeployer {
public:
    PackageDeployer(const BridgeConfig& config, StatusReporter& reporter);

    void deploy();

private:
    const BridgeConfig& config_;
    StatusReporter& reporter_;
    std::unique_ptr<SignatureVerifier> verifier_;

    void deploy_file(const std::wstring& src, const std::wstring& dst);
    bool validate_signature(const std::wstring& dll_path, const std::string& expected_sig);
    bool needs_deployment(const std::wstring& src, const std::wstring& dst);
    void restore_on_failure(const std::wstring& deployed_path);
};

} // namespace bridge
