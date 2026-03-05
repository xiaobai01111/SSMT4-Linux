#pragma once

#include <string>
#include <vector>
#include <cstdint>

#ifdef _WIN32
#include <windows.h>
#include <bcrypt.h>
#endif

namespace bridge {

class SignatureVerifier {
public:
    explicit SignatureVerifier(const std::string& base64_public_key);
    ~SignatureVerifier();

    SignatureVerifier(const SignatureVerifier&) = delete;
    SignatureVerifier& operator=(const SignatureVerifier&) = delete;

    bool verify(const std::string& base64_signature,
                const std::vector<uint8_t>& data);

private:
#ifdef _WIN32
    BCRYPT_KEY_HANDLE key_handle_ = nullptr;
    BCRYPT_ALG_HANDLE sign_alg_ = nullptr;
    BCRYPT_ALG_HANDLE hash_alg_ = nullptr;
#endif

    std::vector<uint8_t> base64_decode(const std::string& input);
    void import_public_key(const std::vector<uint8_t>& der_key);
};

} // namespace bridge
