import Testing
@testable import MikomaiDesktopCore

@Suite
struct NetworkUtilityPolicyTests {
    @Test func savedDiagnosticHostsResolveBeforeDNS() throws {
        let device = SavedConnection(sourceID: "gateway-id", name: "NakaokuGW", host: "192.168.50.1")
        for alias in ["NakaokuGW", "nakaokugw", device.id.uuidString, "gateway-id"] {
            #expect(try RegisteredDiagnosticHostPolicy.resolve(alias, connections: [device]) == "192.168.50.1")
        }
        for host in ["192.168.50.1", "2001:db8::1", "unregistered.example"] {
            #expect(try RegisteredDiagnosticHostPolicy.resolve(host, connections: [device]) == host)
        }
        let missing = SavedConnection(name: "NakaokuGW", host: " ")
        #expect(throws: RegisteredDiagnosticHostPolicy.ResolutionError.self) {
            try RegisteredDiagnosticHostPolicy.resolve("NakaokuGW", connections: [missing])
        }
        let duplicate = SavedConnection(name: "nakaokugw", host: "192.0.2.1")
        #expect(throws: RegisteredDiagnosticHostPolicy.ResolutionError.self) {
            try RegisteredDiagnosticHostPolicy.resolve("NakaokuGW", connections: [device, duplicate])
        }
    }

    @Test func arpUsesFormerTauriVendorCommands() {
        for type in ["Cisco IOS", "cisco_ios", "arista_eos", "furukawa_fitelnet"] {
            #expect(ARPCommandPolicy.command(for: type) == "show ip arp")
        }
        for type in ["juniper_junos", "yamaha"] {
            #expect(ARPCommandPolicy.command(for: type) == "show arp")
        }
    }

    @Test func recognizesPublicIPv4ButNotPrivateOrSpecialRanges() {
        for address in ["8.8.8.8", "1.1.1.1", "198.51.100.1"] {
            #expect(IPAddressPolicy.isGlobalIP(address))
        }
        for address in [
            "127.0.0.1", "10.0.0.1", "172.16.0.1", "172.31.255.255", "192.168.1.1",
            "169.254.10.10", "0.0.0.0", "224.0.0.1", "255.255.255.255"
        ] {
            #expect(!IPAddressPolicy.isGlobalIP(address))
        }
    }

    @Test func recognizesPublicIPv6ButNotLocalPrivateOrMulticastAddresses() {
        for address in ["2001:db8::1", "2001:4860:4860::8888"] {
            #expect(IPAddressPolicy.isGlobalIP(address))
        }
        for address in ["::1", "fe80::1", "fc00::1", "ff02::1"] {
            #expect(!IPAddressPolicy.isGlobalIP(address))
        }
    }

    @Test func rejectsInvalidAddresses() {
        for address in ["invalid-ip", "999.999.999.999", "256.0.0.1"] {
            #expect(!IPAddressPolicy.isGlobalIP(address))
        }
    }

    @Test func parsesPingCommandAndJapaneseHostPhrase() {
        #expect(PingCommandParser.parse("ping 192.168.1.1") == PingCommand(host: "192.168.1.1"))
        #expect(PingCommandParser.parse("10.0.0.1へピン") == PingCommand(host: "10.0.0.1"))
        #expect(PingCommandParser.parse("hello world") == nil)
    }

    @Test func parsesSizeCountAndFragmentFlags() {
        #expect(PingCommandParser.parse("ping localhost size 100") == PingCommand(host: "localhost", size: 100))
        #expect(PingCommandParser.parse("ping 8.8.8.8 5回実行") == PingCommand(host: "8.8.8.8", count: 5))
        #expect(PingCommandParser.parse("ping 1.1.1.1 フラグメント禁止") == PingCommand(host: "1.1.1.1", df: true))
    }

    @Test func mapsPortableAgentPingArgumentsToSafeMacOSFlags() {
        #expect(PingCommand(host: "192.0.2.1", size: 1200, count: 3, df: true).processArguments == ["-c", "3", "-s", "1200", "-D", "192.0.2.1"])
        #expect(PingCommand(host: "192.0.2.1", size: 65_501).processArguments == nil)
        #expect(PingCommand(host: "-c").processArguments == nil)
    }

    @Test func cpuWatchUsesLegacyVendorCommandsAndRequiresNumericUsage() {
        #expect(CPUUsagePolicy.command(for: "juniper_junos") == "show system processes extensive | match CPU")
        #expect(CPUUsagePolicy.command(for: "arista_eos") == "show processes top once")
        #expect(CPUUsagePolicy.command(for: "yamaha") == "show status cpu")
        #expect(CPUUsagePolicy.command(for: "furukawa_fitelnet") == "show cpu")
        #expect(CPUUsagePolicy.command(for: "cisco_ios") == "show processes cpu")
        #expect(CPUUsagePolicy.parse("CPU utilization for five seconds: 82%/10%") == 82)
        #expect(CPUUsagePolicy.parse("CPU utilization: 17 percent") == 17)
        #expect(CPUUsagePolicy.parse("CPU utilization: 101%") == nil)
        #expect(CPUUsagePolicy.parse("no CPU data") == nil)
    }

    @Test func detectsNetworkDeviceErrorsInOutput() {
        #expect(NetworkCommandOutputPolicy.hasError(in: "% Invalid input detected at '^' marker."))
        #expect(NetworkCommandOutputPolicy.hasError(in: "% Incomplete command."))
        #expect(NetworkCommandOutputPolicy.hasError(in: "% Ambiguous command: \"sh\""))
        #expect(NetworkCommandOutputPolicy.hasError(in: "syntax error, unexpected end of line"))
        #expect(NetworkCommandOutputPolicy.hasError(in: "Netmiko error: connection timed out"))
        #expect(NetworkCommandOutputPolicy.hasError(in: "error: device not reachable"))
        #expect(!NetworkCommandOutputPolicy.hasError(in: "Building configuration...\n[OK]"))
        #expect(!NetworkCommandOutputPolicy.hasError(in: "hostname switch-01"))
        
        #expect(NetworkCommandOutputPolicy.detectError(in: "test % invalid input here") == .invalidInput("% invalid input"))
        #expect(NetworkCommandOutputPolicy.detectError(in: "test syntax error here") == .syntaxError("syntax error"))
    }
}

