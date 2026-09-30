import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct HostSuggestionPolicyTests {
    private let labels = HostSuggestionLabels(localhost: "localhost", pastIps: "past")

    @Test func findsMatchingSavedHostsAndOnlyNewMatchingRecentIPs() {
        let result = HostSuggestionPolicy.find(
            query: "10",
            availableHosts: [HostSuggestion(hostname: "router", ip: "10.0.0.1")],
            recentIPs: ["10.0.0.1", "10.0.0.2"],
            labels: labels
        )

        #expect(result == [
            HostSuggestion(hostname: "router", ip: "10.0.0.1"),
            HostSuggestion(hostname: "10.0.0.2", ip: "past")
        ])
    }

    @Test func localhostAndHostnameMatchingAreCaseInsensitive() {
        let result = HostSuggestionPolicy.find(
            query: "LOCAL",
            availableHosts: [HostSuggestion(hostname: "Local-Edge", ip: "192.0.2.5")],
            recentIPs: [],
            labels: labels
        )

        #expect(result == [
            HostSuggestion(hostname: "localhost", ip: "localhost"),
            HostSuggestion(hostname: "Local-Edge", ip: "192.0.2.5")
        ])
    }

    @Test func recentHostUpdatesPrependDeduplicateAndCapAtTen() {
        let current = (1...10).map { "192.0.2.\($0)" }
        let result = HostSuggestionPolicy.updateRecentHosts(
            ["198.51.100.1", current[0], "198.51.100.2"],
            current: current
        )

        let expected = (["198.51.100.1", current[0], "198.51.100.2"] + current.dropFirst(1)).prefix(10)
        #expect(result == Array(expected))
        #expect(HostSuggestionPolicy.updateRecentHosts([], current: current) == current)
        #expect(HostSuggestionPolicy.updateRecentHosts([current[0]], current: current) == current)
    }
}
