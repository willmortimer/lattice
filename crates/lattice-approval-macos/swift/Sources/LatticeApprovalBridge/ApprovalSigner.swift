import CryptoKit
import Foundation
import Security

enum ApprovalBridgeError: Error {
    case secureEnclaveUnavailable(String)
    case keychain(OSStatus)
    case signingFailed(String)
    case notLoaded
}

enum ApprovalKeyIDs {
    static let algorithm = "ES256"
    static let backend = "secure-enclave"
    static let keychainService = "dev.lattice.desktop.approval"
    static let keychainAccount = "se-p256"
    static let deviceIDAccount = "device-id"
    /// Must match Entitlements.plist keychain-access-groups.
    static let accessGroup = "PQNKMDU3U3.group.dev.lattice.shared"

    static func keyID(forPublicKeyDER der: Data) -> String {
        let digest = SHA256.hash(data: der)
        let hex = digest.prefix(8).map { String(format: "%02x", $0) }.joined()
        return "key_\(hex)"
    }
}

final class SecureEnclaveApprovalSigner: @unchecked Sendable {
    private let privateKey: SecureEnclave.P256.Signing.PrivateKey
    let deviceID: String
    let keyID: String

    static func loadOrCreate() throws -> SecureEnclaveApprovalSigner {
        let deviceID = try loadOrCreateDeviceID()
        if let existing = try loadPrivateKeyData() {
            do {
                let key = try SecureEnclave.P256.Signing.PrivateKey(dataRepresentation: existing)
                return SecureEnclaveApprovalSigner(privateKey: key, deviceID: deviceID)
            } catch {
                throw ApprovalBridgeError.secureEnclaveUnavailable(
                    "stored SE key could not be restored: \(error.localizedDescription)"
                )
            }
        }
        let key: SecureEnclave.P256.Signing.PrivateKey
        do {
            key = try SecureEnclave.P256.Signing.PrivateKey()
        } catch {
            throw ApprovalBridgeError.secureEnclaveUnavailable(error.localizedDescription)
        }
        try storePrivateKeyData(key.dataRepresentation)
        return SecureEnclaveApprovalSigner(privateKey: key, deviceID: deviceID)
    }

    private init(privateKey: SecureEnclave.P256.Signing.PrivateKey, deviceID: String) {
        self.privateKey = privateKey
        self.deviceID = deviceID
        self.keyID = ApprovalKeyIDs.keyID(forPublicKeyDER: privateKey.publicKey.derRepresentation)
    }

    func sign(payload: Data) throws -> Data {
        do {
            return try privateKey.signature(for: payload).derRepresentation
        } catch {
            throw ApprovalBridgeError.signingFailed(error.localizedDescription)
        }
    }

    private static func loadOrCreateDeviceID() throws -> String {
        if let existing = try readKeychainUTF8(account: ApprovalKeyIDs.deviceIDAccount) {
            return existing
        }
        let created =
            "device_"
            + UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: "")
        try writeKeychainUTF8(account: ApprovalKeyIDs.deviceIDAccount, value: created)
        return created
    }

    private static func loadPrivateKeyData() throws -> Data? {
        try readKeychainData(account: ApprovalKeyIDs.keychainAccount)
    }

    private static func storePrivateKeyData(_ data: Data) throws {
        try writeKeychainData(account: ApprovalKeyIDs.keychainAccount, data: data)
    }
}

private func baseQuery(account: String) -> [String: Any] {
    [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: ApprovalKeyIDs.keychainService,
        kSecAttrAccount as String: account,
        kSecAttrAccessGroup as String: ApprovalKeyIDs.accessGroup,
        kSecUseDataProtectionKeychain as String: true,
    ]
}

private func readKeychainData(account: String) throws -> Data? {
    var query = baseQuery(account: account)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var item: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &item)
    if status == errSecItemNotFound {
        return nil
    }
    guard status == errSecSuccess else {
        throw ApprovalBridgeError.keychain(status)
    }
    guard let data = item as? Data else {
        throw ApprovalBridgeError.keychain(errSecDecode)
    }
    return data
}

private func writeKeychainData(account: String, data: Data) throws {
    let query = baseQuery(account: account)
    let attributes: [String: Any] = [
        kSecValueData as String: data,
        kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    ]
    let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    if updateStatus == errSecSuccess {
        return
    }
    if updateStatus != errSecItemNotFound {
        throw ApprovalBridgeError.keychain(updateStatus)
    }
    var addQuery = query
    addQuery[kSecValueData as String] = data
    addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
    let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
    guard addStatus == errSecSuccess else {
        throw ApprovalBridgeError.keychain(addStatus)
    }
}

private func readKeychainUTF8(account: String) throws -> String? {
    guard let data = try readKeychainData(account: account) else {
        return nil
    }
    return String(data: data, encoding: .utf8)
}

private func writeKeychainUTF8(account: String, value: String) throws {
    guard let data = value.data(using: .utf8) else {
        throw ApprovalBridgeError.signingFailed("utf8 encode failed")
    }
    try writeKeychainData(account: account, data: data)
}

enum SignerRegistry {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var signer: SecureEnclaveApprovalSigner?

    static func loadOrCreate() throws {
        lock.lock()
        defer { lock.unlock() }
        if signer == nil {
            signer = try SecureEnclaveApprovalSigner.loadOrCreate()
        }
    }

    static func clear() {
        lock.lock()
        defer { lock.unlock() }
        signer = nil
    }

    static func current() throws -> SecureEnclaveApprovalSigner {
        lock.lock()
        defer { lock.unlock() }
        guard let signer else {
            throw ApprovalBridgeError.notLoaded
        }
        return signer
    }
}
