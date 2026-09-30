import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct ConnectionInventoryPolicyTests {
    @Test func saveAddsAndUpdatesByIDButRejectsInvalidRecords() {
        let original = SavedConnection(name: "router", host: "192.0.2.1")
        let added = SavedConnection(name: "switch", host: "192.0.2.2")
        var records = ConnectionInventoryPolicy.saving(original, into: [])
        records = ConnectionInventoryPolicy.saving(added, into: records)
        #expect(records.count == 2)

        var updated = original
        updated.host = "192.0.2.10"
        records = ConnectionInventoryPolicy.saving(updated, into: records)
        #expect(records.count == 2)
        #expect(records.first(where: { $0.id == original.id })?.host == "192.0.2.10")

        var invalid = original
        invalid.name = "not a valid name"
        #expect(ConnectionInventoryPolicy.saving(invalid, into: records) == records)
    }

    @Test func removeDeletesOnlyMatchingRecord() {
        let first = SavedConnection(name: "router", host: "192.0.2.1")
        let second = SavedConnection(name: "switch", host: "192.0.2.2")

        let remaining = ConnectionInventoryPolicy.removing(first.id, from: [first, second])

        #expect(remaining == [second])
        #expect(ConnectionInventoryPolicy.removing(UUID(), from: remaining) == remaining)
    }

    @Test func credentialMetadataPreservesClearsAndSetsPresence() {
        var connection = SavedConnection(name: "router", host: "192.0.2.1", hasPassword: true, hasEnablePassword: true)
        connection = ConnectionCredentialPolicy.applying(password: nil, enablePassword: nil, to: connection)
        #expect(connection.hasPassword)
        #expect(connection.hasEnablePassword)

        connection = ConnectionCredentialPolicy.applying(password: "", enablePassword: "secret", to: connection)
        #expect(!connection.hasPassword)
        #expect(connection.hasEnablePassword)

        connection = ConnectionCredentialPolicy.applying(password: "secret", enablePassword: "", to: connection)
        #expect(connection.hasPassword)
        #expect(!connection.hasEnablePassword)
    }

    @Test func editorValidationAcceptsIPv6AndRejectsInvalidHostPortAndControlCharacters() {
        let valid = SavedConnection(name: "router-1", host: "2001:db8::1", port: "65535", username: "netadmin")
        #expect(valid.validationError == nil)

        var invalid = valid
        invalid.host = "not a host"
        #expect(invalid.validationError != nil)
        invalid = valid
        invalid.port = "65536"
        #expect(invalid.validationError != nil)
        invalid = valid
        invalid.username = "admin\n"
        #expect(invalid.validationError != nil)
    }

    @Test func consoleSettingsRoundTripThroughNativeSettingsCodec() throws {
        let settings = DesktopSettings(consolePort: "/dev/tty.usbserial-A", consoleBaudRate: 115200)
        let decoded = try DesktopSettingsCodec.decode(DesktopSettingsCodec.encode(settings))
        #expect(decoded.consolePort == "/dev/tty.usbserial-A")
        #expect(decoded.consoleBaudRate == 115200)

        var patch = DesktopSettingsPatch()
        patch.consolePort = .set(nil)
        patch.consoleBaudRate = .set(9600)
        var merged = decoded
        merged.merge(patch)
        #expect(merged.consolePort == nil)
        #expect(merged.consoleBaudRate == 9600)
    }
}
