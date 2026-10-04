import Testing
@testable import MikomaiDesktopCore

@Suite struct InterfaceObservationPolicyTests {
    @Test func yamahaRequiresOneLANAndKeepsReadOnlyCommand() throws {
        #expect(try InterfaceObservationPolicy.command(deviceType: "yamaha", interface: "LAN1") == "show status lan1")
        #expect(throws: InterfaceObservationError.self) {
            try InterfaceObservationPolicy.command(deviceType: "yamaha", interface: nil)
        }
        #expect(throws: InterfaceObservationError.self) {
            try InterfaceObservationPolicy.command(deviceType: "yamaha", interface: "lan1\nsave")
        }
        #expect(try InterfaceObservationPolicy.command(deviceType: "cisco_ios", interface: nil) == "show interfaces")
    }
}
