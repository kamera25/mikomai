import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct DeviceTypeCatalogTests {
    @Test func everyCatalogIDSurvivesPersistenceAndTransportResolution() throws {
        for id in DeviceTypeCatalog.deviceTypes {
            let connection = SavedConnection(name: "router", host: "192.0.2.1", deviceType: id)
            let restored = try JSONDecoder().decode(SavedConnection.self, from: JSONEncoder().encode(connection))
            #expect(restored.deviceType == id)
            #expect(DeviceTypeCatalog.canonicalID(for: restored.deviceType) == id)
            let csv = try ConnectionCSVCodec.exportCSV([connection])
            let imported = try ConnectionCSVCodec.importCSV(csv, existing: []).connections
            #expect(imported.first?.deviceType == id)
        }
    }

    @Test func searchMatchesAliasesAndIDs() {
        #expect(DeviceTypeCatalog.matching("UNIVERGE") == ["nec_ix"])
        #expect(DeviceTypeCatalog.matching("  CISCO_ASA  ") == ["cisco_asa"])
        #expect(DeviceTypeCatalog.matching("SEIL/OS") == ["iij_seilos"])
        #expect(DeviceTypeCatalog.matching("no-such-device").isEmpty)
        #expect(DeviceTypeCatalog.matching("") == DeviceTypeCatalog.deviceTypes)
        #expect(DeviceTypeCatalog.optionLabel(for: "cisco_xr") == "Cisco IOS-XR (cisco_xr)")
    }

    @Test func oldSwiftNamesResolveWithoutCollapsingDistinctDrivers() {
        #expect(DeviceTypeCatalog.canonicalID(for: "Cisco IOS") == "cisco_ios")
        #expect(DeviceTypeCatalog.canonicalID(for: "Juniper JunOS") == "juniper_junos")
        #expect(DeviceTypeCatalog.canonicalID(for: "Other") == "generic")
        #expect(DeviceTypeCatalog.canonicalID(for: "F220") == "furukawa_fitelnet")
        for id in ["cisco_asa", "cisco_xr", "juniper_screenos", "aruba_aoscx"] {
            let connection = SavedConnection(name: "router", host: "192.0.2.1", connectionType: "Telnet", deviceType: id)
            #expect(connection.transportDeviceType(DeviceTypeCatalog.canonicalID(for: id)) == id + "_telnet")
        }
        #expect(DeviceTypeCatalog.displayName(for: "custom_driver") == "custom_driver")
    }
}
