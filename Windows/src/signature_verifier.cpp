#include "signature_verifier.h"
#include <stdexcept>
#include <cstring>

#ifdef _WIN32
#include <wincrypt.h>
#pragma comment(lib, "bcrypt.lib")
#pragma comment(lib, "crypt32.lib")

#ifndef NT_SUCCESS
#define NT_SUCCESS(Status) (((NTSTATUS)(Status)) >= 0)
#endif
#endif

namespace bridge {

SignatureVerifier::SignatureVerifier(const std::string& base64_public_key) {
    if (base64_public_key.empty()) return;
    auto der_key = base64_decode(base64_public_key);
    if (!der_key.empty()) {
        import_public_key(der_key);
    }
}

SignatureVerifier::~SignatureVerifier() {
#ifdef _WIN32
    if (key_handle_) BCryptDestroyKey(key_handle_);
    if (sign_alg_) BCryptCloseAlgorithmProvider(sign_alg_, 0);
    if (hash_alg_) BCryptCloseAlgorithmProvider(hash_alg_, 0);
#endif
}

std::vector<uint8_t> SignatureVerifier::base64_decode(const std::string& input) {
    std::vector<uint8_t> result;
#ifdef _WIN32
    DWORD size = 0;
    if (!CryptStringToBinaryA(input.c_str(), (DWORD)input.size(),
                               CRYPT_STRING_BASE64, nullptr, &size, nullptr, nullptr)) {
        return result;
    }
    result.resize(size);
    if (!CryptStringToBinaryA(input.c_str(), (DWORD)input.size(),
                               CRYPT_STRING_BASE64, result.data(), &size, nullptr, nullptr)) {
        result.clear();
    }
    result.resize(size);
#endif
    return result;
}

void SignatureVerifier::import_public_key(const std::vector<uint8_t>& der_key) {
#ifdef _WIN32
    // Open ECDSA P-384 algorithm provider
    NTSTATUS status = BCryptOpenAlgorithmProvider(&sign_alg_,
        BCRYPT_ECDSA_P384_ALGORITHM, nullptr, 0);
    if (!NT_SUCCESS(status)) {
        throw std::runtime_error("Failed to open ECDSA P-384 algorithm provider");
    }

    // Open SHA-256 hash algorithm
    status = BCryptOpenAlgorithmProvider(&hash_alg_,
        BCRYPT_SHA256_ALGORITHM, nullptr, 0);
    if (!NT_SUCCESS(status)) {
        throw std::runtime_error("Failed to open SHA-256 algorithm provider");
    }

    // The DER key is SubjectPublicKeyInfo format for EC P-384
    // Parse the DER to extract the raw EC point (uncompressed, 97 bytes: 04 + 48x + 48y)
    // SubjectPublicKeyInfo ::= SEQUENCE { algorithm, subjectPublicKey BIT STRING }
    // We need to find the BIT STRING containing the EC point

    // Simple DER parser: find the 0x04 prefix of uncompressed EC point
    // P-384 point = 1 byte prefix (0x04) + 48 bytes X + 48 bytes Y = 97 bytes
    const size_t ec_point_size = 97;
    const uint8_t* ec_point = nullptr;

    for (size_t i = 0; i + ec_point_size <= der_key.size(); ++i) {
        if (der_key[i] == 0x04 && (der_key.size() - i) >= ec_point_size) {
            // Verify this looks like an uncompressed point at the right offset
            // The BIT STRING in DER has a leading 0x00 unused bits byte
            if (i > 0 && der_key[i - 1] == 0x00) {
                ec_point = &der_key[i];
                break;
            }
        }
    }

    if (!ec_point) {
        throw std::runtime_error("Failed to parse EC public key from DER");
    }

    // Build BCRYPT_ECCKEY_BLOB + raw X,Y coordinates
    // BCRYPT_ECCKEY_BLOB: { Magic, cbKey } followed by X and Y
    const ULONG coord_size = 48; // P-384 = 48 bytes per coordinate
    ULONG blob_size = sizeof(BCRYPT_ECCKEY_BLOB) + coord_size * 2;
    std::vector<uint8_t> blob(blob_size);

    BCRYPT_ECCKEY_BLOB* header = reinterpret_cast<BCRYPT_ECCKEY_BLOB*>(blob.data());
    header->dwMagic = BCRYPT_ECDSA_PUBLIC_P384_MAGIC;
    header->cbKey = coord_size;

    // Copy X and Y (skip the 0x04 prefix)
    std::memcpy(blob.data() + sizeof(BCRYPT_ECCKEY_BLOB), ec_point + 1, coord_size * 2);

    status = BCryptImportKeyPair(sign_alg_, nullptr, BCRYPT_ECCPUBLIC_BLOB,
                                  &key_handle_, blob.data(), (ULONG)blob.size(), 0);
    if (!NT_SUCCESS(status)) {
        throw std::runtime_error("Failed to import EC public key");
    }
#endif
}

bool SignatureVerifier::verify(const std::string& base64_signature,
                                const std::vector<uint8_t>& data) {
#ifdef _WIN32
    if (!key_handle_ || !hash_alg_) return false;

    auto signature = base64_decode(base64_signature);
    if (signature.empty()) return false;

    // Hash the data with SHA-256
    BCRYPT_HASH_HANDLE hash_handle = nullptr;
    NTSTATUS status = BCryptCreateHash(hash_alg_, &hash_handle, nullptr, 0, nullptr, 0, 0);
    if (!NT_SUCCESS(status)) return false;

    status = BCryptHashData(hash_handle, const_cast<PUCHAR>(data.data()), (ULONG)data.size(), 0);
    if (!NT_SUCCESS(status)) {
        BCryptDestroyHash(hash_handle);
        return false;
    }

    // Get hash length
    DWORD hash_size = 0;
    ULONG result_size = 0;
    BCryptGetProperty(hash_alg_, BCRYPT_HASH_LENGTH, (PUCHAR)&hash_size,
                       sizeof(hash_size), &result_size, 0);

    std::vector<uint8_t> hash(hash_size);
    status = BCryptFinishHash(hash_handle, hash.data(), hash_size, 0);
    BCryptDestroyHash(hash_handle);
    if (!NT_SUCCESS(status)) return false;

    // The signature from Python (ecdsa library) is DER-encoded
    // BCrypt expects P1363 format (r || s, each 48 bytes for P-384)
    // We may need to convert DER → P1363
    std::vector<uint8_t> p1363_sig;

    if (signature.size() > 48 * 2 + 6 && signature[0] == 0x30) {
        // DER encoded: SEQUENCE { INTEGER r, INTEGER s }
        size_t pos = 2;
        if (signature[1] & 0x80) pos++; // long form length

        // Parse r
        if (signature[pos] != 0x02) return false;
        pos++;
        size_t r_len = signature[pos++];
        const uint8_t* r_data = &signature[pos];
        pos += r_len;

        // Parse s
        if (signature[pos] != 0x02) return false;
        pos++;
        size_t s_len = signature[pos++];
        const uint8_t* s_data = &signature[pos];

        // Convert to P1363 (pad/trim to 48 bytes each)
        p1363_sig.resize(96, 0);
        if (r_len > 48) { r_data += (r_len - 48); r_len = 48; }
        if (s_len > 48) { s_data += (s_len - 48); s_len = 48; }
        std::memcpy(p1363_sig.data() + (48 - r_len), r_data, r_len);
        std::memcpy(p1363_sig.data() + 48 + (48 - s_len), s_data, s_len);
    } else if (signature.size() == 96) {
        // Already P1363 format
        p1363_sig = signature;
    } else {
        return false;
    }

    // Verify
    status = BCryptVerifySignature(key_handle_, nullptr, hash.data(), hash_size,
                                    p1363_sig.data(), (ULONG)p1363_sig.size(), 0);
    return NT_SUCCESS(status);
#else
    return false;
#endif
}

} // namespace bridge
