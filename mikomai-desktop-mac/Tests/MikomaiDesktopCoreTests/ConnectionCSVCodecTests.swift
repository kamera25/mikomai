import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct ConnectionCSVCodecTests {
    @Test func importsValidRowsAndWarnsForInvalidRowsWithoutStoppingTheImport() throws {
        let csv = """
        id,status,hostname,ip,port,type,lastConnected,deviceType,vendorType,username,password
        r1,online,router-1,192.0.2.1,22,Cisco IOS (SSH),Never,cisco_ios,Cisco,admin,do-not-import
        ,offline,router<script>,192.0.2.2,22,SSH,Never,cisco_ios,,admin,
        ,offline,router-2,192.0.2.2,0,SSH,Never,cisco_ios,,admin,
        ,offline,router-3,192.0.2.3,23,FTP,Never,cisco_ios,,admin,
        ,offline,router-4,192.0.2.4,23,Telnet,Never,cisco_ios,,admin,
        ,offline,,,22,SSH,Never,cisco_ios,,admin,
        ,offline,router-5,192.0.2.5,22,SSH,Never,cisco_ios,,"admin
        evil",
        ,offline,router-6,999.999.999.999; rm -rf,22,SSH,Never,cisco_ios,,admin,
        """

        let result = try ConnectionCSVCodec.importCSV(csv, existing: [])

        #expect(result.importedCount == 2)
        #expect(result.warnings.map(\.row) == [3, 4, 5, 7, 8, 9])
        #expect(result.connections.count == 2)
        #expect(result.connections[0].sourceID == "r1")
        #expect(result.connections[0].connectionType == "SSH")
        #expect(result.connections[0].username == "admin")
        #expect(!result.connections[0].hasPassword)
        #expect(result.connections[1].connectionType == "Telnet")
    }

    @Test func duplicateIDsUpdateTheExistingRecordAndLastCsvRowWins() throws {
        let existing = [SavedConnection(sourceID: "router-id", name: "old-router", host: "192.0.2.1", username: "old-user", hasPassword: true)]
        let csv = """
        id,hostname,ip,type,username
        router-id,first-router,192.0.2.2,SSH,first-user
        router-id,last-router,192.0.2.3,Console,last-user
        """

        let result = try ConnectionCSVCodec.importCSV(csv, existing: existing)

        #expect(result.importedCount == 2)
        #expect(result.warnings.isEmpty)
        #expect(result.connections.count == 1)
        #expect(result.connections[0].name == "last-router")
        #expect(result.connections[0].host == "192.0.2.3")
        #expect(result.connections[0].connectionType == "Console")
        #expect(result.connections[0].hasPassword)
        #expect(result.connections[0].username == "last-user")
    }

    @Test func missingIDsAreGeneratedAndMalformedCSVIsRejected() throws {
        let csv = "hostname,ip\nrouter-1,192.0.2.1"
        let result = try ConnectionCSVCodec.importCSV(csv, existing: [])

        #expect(result.importedCount == 1)
        #expect(result.connections[0].sourceID != nil)
        #expect(!result.connections[0].sourceID!.isEmpty)
        #expect(result.connections[0].connectionType == "SSH")
        do {
            try ConnectionCSVCodec.importCSV("hostname,ip\n\"router-1,192.0.2.1", existing: [])
            Issue.record("Expected an unterminated quoted value to fail")
        } catch is ConnectionCSVError {
        } catch {
            Issue.record("Unexpected parse error: \(error)")
        }
    }

    @Test func exportUsesTauriHeadersEscapesFieldsAndExcludesCredentials() throws {
        let connection = SavedConnection(
            sourceID: "router-1",
            name: "router-1",
            host: "192.0.2.1",
            username: "ops, \"primary\"",
            deviceType: "Cisco, IOS",
            hasPassword: true,
            hasEnablePassword: true
        )
        let csv = try ConnectionCSVCodec.exportCSV([connection])
        let result = try ConnectionCSVCodec.importCSV(csv, existing: [])

        #expect(csv.hasPrefix("id,status,hostname,ip,port,type,lastConnected,deviceType,vendorType,username\n"))
        #expect(!csv.lowercased().contains("password"))
        #expect(result.connections.count == 1)
        #expect(result.connections[0].username == "ops, \"primary\"")
        #expect(result.connections[0].deviceType == "Cisco, IOS")
        #expect(result.connections[0].sourceID == "router-1")
    }

    @Test func acceptsUtf8BomHeadersAndTauriFieldNamesCaseInsensitively() throws {
        let csv = "\u{FEFF}HOSTNAME,IP,TYPE,PORT\r\nrouter-1,192.0.2.1,serial-port,9600\r\n"
        let result = try ConnectionCSVCodec.importCSV(csv, existing: [])

        #expect(result.importedCount == 1)
        #expect(result.warnings.isEmpty)
        #expect(result.connections.first?.connectionType == "Console")
        #expect(result.connections.first?.port == "9600")
    }
}
