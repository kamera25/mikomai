import Testing
@testable import MikomaiDesktopCore

@Suite
struct NetworkUtilityPolicyTests {
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
}
