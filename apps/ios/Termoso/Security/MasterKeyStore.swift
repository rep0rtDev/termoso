import Foundation
import Security

/// Where the 32-byte vault master key lives. The Rust core encrypts the
/// SQLite vault with it; the key itself never leaves the device.
protocol MasterKeyStore: Sendable {
    /// Returns the existing key or creates and persists a fresh one.
    func loadOrCreate() throws -> Data
    /// Whether a key has been created on this device.
    func exists() throws -> Bool
    /// Forgets the key. The vault becomes unreadable — used for "erase all data".
    func erase() throws
}

enum MasterKeyError: LocalizedError {
    case keychain(OSStatus)
    case corrupt

    var errorDescription: String? {
        switch self {
        case let .keychain(status):
            let message = SecCopyErrorMessageString(status, nil) as String? ?? "OSStatus \(status)"
            return "Keychain error: \(message)"
        case .corrupt:
            return "The stored master key is not 32 bytes."
        }
    }
}

/// Keychain-backed store. The key is a generic-password item bound to this
/// device only (`ThisDeviceOnly`, never synced to iCloud Keychain) and readable
/// after the first unlock so the vault can open in the background.
struct KeychainMasterKeyStore: MasterKeyStore {
    var service = "com.termoso.ios.vault"
    var account = "master-key"

    static let keyLength = 32

    func loadOrCreate() throws -> Data {
        if let existing = try read() {
            return existing
        }
        let key = Self.randomKey()
        try write(key)
        return key
    }

    func exists() throws -> Bool {
        try read() != nil
    }

    func erase() throws {
        let status = SecItemDelete(baseQuery() as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw MasterKeyError.keychain(status)
        }
    }

    private func baseQuery() -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    private func read() throws -> Data? {
        var query = baseQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data, data.count == Self.keyLength else {
                throw MasterKeyError.corrupt
            }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw MasterKeyError.keychain(status)
        }
    }

    private func write(_ key: Data) throws {
        var attributes = baseQuery()
        attributes[kSecValueData as String] = key
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        attributes[kSecAttrSynchronizable as String] = false
        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw MasterKeyError.keychain(status)
        }
    }

    static func randomKey() -> Data {
        var bytes = [UInt8](repeating: 0, count: keyLength)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        precondition(status == errSecSuccess, "SecRandomCopyBytes failed: \(status)")
        return Data(bytes)
    }
}

/// In-memory store for UI tests and previews: a fresh random key per process.
final class EphemeralMasterKeyStore: MasterKeyStore, @unchecked Sendable {
    private let lock = NSLock()
    private var key: Data?

    init() {}

    func loadOrCreate() throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        if let key { return key }
        let fresh = KeychainMasterKeyStore.randomKey()
        key = fresh
        return fresh
    }

    func exists() throws -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return key != nil
    }

    func erase() throws {
        lock.lock()
        defer { lock.unlock() }
        key = nil
    }
}
