import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct ConnectionCredentialPersistenceTests {
    @Test func editorCredentialsSavePreserveClearAndDeleteByConnectionID() {
        let store = FakeCredentialStore()
        let persistence = ConnectionCredentialPersistence(store: store)
        let id = UUID()

        #expect(persistence.load(for: id) == ConnectionCredentials(password: nil, enablePassword: nil))

        _ = persistence.save(for: id, password: "login-secret", enablePassword: "enable-secret")
        #expect(persistence.load(for: id) == ConnectionCredentials(password: "login-secret", enablePassword: "enable-secret"))

        _ = persistence.save(for: id, password: nil, enablePassword: "")
        #expect(persistence.load(for: id) == ConnectionCredentials(password: "login-secret", enablePassword: nil))

        persistence.delete(for: id)
        #expect(persistence.load(for: id) == ConnectionCredentials(password: nil, enablePassword: nil))
        #expect(store.values.isEmpty)
    }
}

private final class FakeCredentialStore: CredentialStore, @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String: String] = [:]

    var values: [String: String] {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func save(key: String, value: String) {
        lock.lock()
        storage[key] = value
        lock.unlock()
    }

    func load(key: String) -> String? {
        lock.lock()
        defer { lock.unlock() }
        return storage[key]
    }

    func delete(key: String) {
        lock.lock()
        storage.removeValue(forKey: key)
        lock.unlock()
    }
}
