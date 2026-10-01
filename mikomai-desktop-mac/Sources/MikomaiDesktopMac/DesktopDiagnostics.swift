import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Diagnostics Runners

@MainActor
final class DiagnosticsRunner: ObservableObject {
    @Published var output = ""
    @Published var isRunning = false
    private var process: Process?

    func run(command: String, arguments: [String]) {
        stop()
        isRunning = true
        output = "$ \(command) \(arguments.joined(separator: " "))\n"

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: command)
        proc.arguments = arguments

        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = pipe

        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let str = String(data: data, encoding: .utf8) else { return }
            Task { @MainActor in
                self?.output += str
            }
        }

        proc.terminationHandler = { [weak self] _ in
            Task { @MainActor in
                self?.isRunning = false
                self?.output += "\n[完了]\n"
                self?.process = nil
            }
        }

        self.process = proc
        do {
            try proc.run()
        } catch {
            output += "エラー: コマンド起動に失敗しました: \(error.localizedDescription)\n"
            isRunning = false
        }
    }

    func stop() {
        if let proc = process, proc.isRunning {
            proc.terminate()
            output += "\n[停止しました]\n"
        }
        process = nil
        isRunning = false
    }

    func clear() {
        output = ""
    }
}

enum NetworkInspector {
    static func fetchArpTable() -> [ArpRecord] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/sbin/arp")
        proc.arguments = ["-an"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            proc.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            return parseArp(text)
        } catch {
            return []
        }
    }

    private static func parseArp(_ text: String) -> [ArpRecord] {
        var records: [ArpRecord] = []
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            guard let openParen = trimmed.firstIndex(of: "("),
                  let closeParen = trimmed.firstIndex(of: ")"),
                  openParen < closeParen else { continue }
            let ip = String(trimmed[trimmed.index(after: openParen)..<closeParen])
            guard let atRange = trimmed.range(of: " at ") else { continue }
            let afterAt = trimmed[atRange.upperBound...]
            guard let onRange = afterAt.range(of: " on ") else { continue }
            let mac = String(afterAt[..<onRange.lowerBound]).trimmingCharacters(in: .whitespaces)
            let afterOn = afterAt[onRange.upperBound...]
            let iface = afterOn.components(separatedBy: .whitespaces).first ?? ""
            let isPermanent = trimmed.contains("permanent")
            let isIncomplete = mac.contains("incomplete")
            records.append(ArpRecord(ip: ip, mac: mac, interface: iface, isPermanent: isPermanent, isIncomplete: isIncomplete))
        }
        return records
    }

    static func fetchRoutingTable() -> [RouteRecord] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/sbin/netstat")
        proc.arguments = ["-rn", "-f", "inet"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            proc.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            return parseRoutes(text)
        } catch {
            return []
        }
    }

    private static func parseRoutes(_ text: String) -> [RouteRecord] {
        var records: [RouteRecord] = []
        var inTable = false
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            if trimmed.hasPrefix("Destination") {
                inTable = true
                continue
            }
            guard inTable else { continue }
            let parts = trimmed.components(separatedBy: .whitespaces).filter { !$0.isEmpty }
            if parts.count >= 4 {
                let dest = parts[0]
                let gateway = parts[1]
                let flags = parts[2]
                let iface = parts[3]
                records.append(RouteRecord(destination: dest, gateway: gateway, flags: flags, interface: iface))
            }
        }
        return records
    }
}

