#include "package_deployer.h"
#include "path_utils.h"
#include "string_utils.h"
#include <memory>
#include <stdexcept>

namespace bridge {

PackageDeployer::PackageDeployer(const BridgeConfig& config, StatusReporter& reporter)
    : config_(config), reporter_(reporter) {
    if (!config.signatures.xxmi_public_key.empty()) {
        verifier_ = std::make_unique<SignatureVerifier>(config.signatures.xxmi_public_key);
    }
}

bool PackageDeployer::needs_deployment(const std::wstring& src, const std::wstring& dst) {
    if (!file_exists(dst)) return true;
    if (!file_exists(src)) return false;

    uint64_t src_size = get_file_size(src);
    uint64_t dst_size = get_file_size(dst);
    if (src_size != dst_size) return true;

    uint64_t src_mtime = get_file_mtime(src);
    uint64_t dst_mtime = get_file_mtime(dst);
    return src_mtime != dst_mtime;
}

void PackageDeployer::deploy_file(const std::wstring& src, const std::wstring& dst) {
    if (!file_exists(src)) {
        throw std::runtime_error("Source DLL not found: " + wide_to_utf8(src));
    }

    // Remove read-only attribute on destination if it exists
    if (file_exists(dst)) {
#ifdef _WIN32
        DWORD attrs = GetFileAttributesW(dst.c_str());
        if (attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_READONLY)) {
            SetFileAttributesW(dst.c_str(), attrs & ~FILE_ATTRIBUTE_READONLY);
        }
#endif
    }

    if (!copy_file_overwrite(src, dst)) {
        throw std::runtime_error("Failed to copy DLL: " + wide_to_utf8(src) +
                                 " -> " + wide_to_utf8(dst));
    }
}

bool PackageDeployer::validate_signature(const std::wstring& dll_path,
                                          const std::string& expected_sig) {
    if (!verifier_) return true; // No verifier = skip verification
    if (expected_sig.empty()) return true;

    auto data = read_file_binary(dll_path);
    if (data.empty()) return false;

    return verifier_->verify(expected_sig, data);
}

void PackageDeployer::restore_on_failure(const std::wstring& deployed_path) {
    if (file_exists(deployed_path)) {
        delete_file(deployed_path);
    }
}

void PackageDeployer::deploy() {
    // DLL files to deploy: from packages_folder to importer_folder
    // The specific DLL names come from the package structure, not hardcoded
    // Standard 3DMigoto DLLs: d3d11.dll, d3dcompiler_47.dll
    struct DllEntry {
        std::wstring name;
    };

    std::vector<DllEntry> dlls = {
        { L"d3d11.dll" },
        { L"d3dcompiler_47.dll" },
    };

    std::vector<std::wstring> deployed;

    for (auto& dll : dlls) {
        std::wstring src = path_join(config_.paths.packages_folder, dll.name);
        std::wstring dst = path_join(config_.paths.importer_folder, dll.name);

        if (!file_exists(src)) {
            reporter_.log("debug", "Package DLL not found, skipping: " + wide_to_utf8(dll.name));
            continue;
        }

        if (needs_deployment(src, dst)) {
            reporter_.log("info", "Deploying " + wide_to_utf8(dll.name));
            try {
                deploy_file(src, dst);
                deployed.push_back(dst);
            } catch (const std::exception& e) {
                // Rollback previously deployed files
                for (auto& d : deployed) {
                    restore_on_failure(d);
                }
                throw;
            }
        }

        // Validate signature if available
        std::string dll_name_utf8 = wide_to_utf8(dll.name);
        auto sig_it = config_.signatures.deployed_migoto_signatures.find(dll_name_utf8);
        if (sig_it != config_.signatures.deployed_migoto_signatures.end()) {
            if (!validate_signature(dst, sig_it->second)) {
                if (!config_.migoto.unsafe_mode) {
                    // Rollback
                    for (auto& d : deployed) {
                        restore_on_failure(d);
                    }
                    throw std::runtime_error("Signature verification failed for " + dll_name_utf8);
                } else {
                    reporter_.warning("Signature mismatch for " + dll_name_utf8 +
                                      " (unsafe mode, continuing)");
                }
            }
        }
    }
}

} // namespace bridge
