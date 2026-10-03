import Foundation
import MikomaiDesktopCore

extension DesktopModel {
    // MARK: - Tool & Subprocess Runners

    nonisolated static func runNetworkWrapper(_ request: NetworkRunnerRequest) -> NetworkOperationOutput {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let executableResources = Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("../Resources/netmiko_wrapper").standardizedFileURL
        let environmentWrapper = ProcessInfo.processInfo.environment["MIKOMAI_NETMIKO_WRAPPER"].map { URL(fileURLWithPath: $0) }
        let binaries = [
            environmentWrapper,
            Bundle.main.resourceURL?.appendingPathComponent("netmiko_wrapper"),
            executableResources,
            cwd.appendingPathComponent("mikomai-core/assets/bin/netmiko_wrapper-macos-arm64"),
        ].compactMap { $0 }
        let scriptCandidates = [
            Bundle.main.resourceURL?.appendingPathComponent("network/netmiko_wrapper.py"),
            cwd.appendingPathComponent("mikomai-core/assets/network/netmiko_wrapper.py")
        ].compactMap { $0 }
        let script = scriptCandidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
        let process = Process()
        if let binary = binaries.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) {
            process.executableURL = binary
            process.arguments = ["--stdin"]
        } else if let script {
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["python3", script.path, "--stdin"]
        } else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "Netmiko 実行ツールが見つかりません。")
        }

        let input = Pipe()
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-net-\(UUID().uuidString)", isDirectory: true)
        do { try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true) }
        catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "一時ログ領域を作成できません: \(error.localizedDescription)") }
        let outURL = tempDirectory.appendingPathComponent("stdout.log")
        let errURL = tempDirectory.appendingPathComponent("stderr.log")
        FileManager.default.createFile(atPath: outURL.path, contents: nil)
        FileManager.default.createFile(atPath: errURL.path, contents: nil)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }
        guard let out = try? FileHandle(forWritingTo: outURL), let err = try? FileHandle(forWritingTo: errURL) else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "ログを作成できません。")
        }
        process.standardInput = input
        process.standardOutput = out
        process.standardError = err
        do {
            try process.run()
            let payload: [String: Any] = [
                "action": request.action,
                "host": request.host,
                "username": request.username,
                "password": request.password,
                "secret": request.secret,
                "device_type": request.deviceType,
                "commands": request.commands,
                "port": request.port
            ]
            let data = try JSONSerialization.data(withJSONObject: payload)
            input.fileHandleForWriting.write(data)
            input.fileHandleForWriting.write(Data([0x0a]))
            input.fileHandleForWriting.closeFile()
            process.waitUntilExit()
            try? out.close()
            try? err.close()
            let stdout = (try? String(contentsOf: outURL, encoding: .utf8)) ?? ""
            let stderr = (try? String(contentsOf: errURL, encoding: .utf8)) ?? ""
            return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
        } catch {
            process.terminate()
            try? out.close(); try? err.close(); input.fileHandleForWriting.closeFile()
            return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription)
        }
    }

    nonisolated static func runPortableAgentTool(
        tool: String,
        target: PortableDeviceTarget,
        arguments: [String: Any],
        connections: [SavedConnection],
        credentialPersistence: ConnectionCredentialPersistence
    ) -> NetworkOperationOutput {
        var arguments = arguments
        if ["self_network_ping", "self_network_traceroute", "self_network_test_connection", "self_network_test_net_connection"].contains(tool),
           let requestedHost = arguments["host"] as? String {
            do {
                arguments["host"] = try RegisteredDiagnosticHostPolicy.resolve(requestedHost, connections: connections)
            } catch {
                return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription)
            }
        }
        if tool == "get_state", target.hostname == "localhost", arguments["resource"] as? String == "arp" {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/sbin/arp")
            process.arguments = ["-an"]
            let out = Pipe(); let err = Pipe()
            process.standardOutput = out; process.standardError = err
            do {
                try process.run(); process.waitUntilExit()
                let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
            } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
        }
        if tool == "validate_cisco_config" || tool == "convert_cisco_config" {
            guard let script = portableAsset("network/config_helper.py"),
                  let python = portablePython() else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "Config helperまたはPython runtimeが見つかりません。")
            }
            let payload: [String: Any] = [
                "action": tool == "validate_cisco_config" ? "validate" : "convert",
                "config": arguments["config"] as? String ?? "",
                "target_vendor": arguments["target_vendor"] as? String ?? arguments["targetVendor"] as? String ?? "juniper"
            ]
            return runJSONPython(script: script, python: python, payload: payload)
        }
        if tool == "self_network_nwdiag" {
            guard let wrapper = portableAsset("network/nwdiag_wrapper.py"),
                  let python = portablePython(),
                  let schema = arguments["schema"] as? String ?? arguments["nwdiag"] as? String else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "nwdiag wrapper、Python runtime、またはschemaがありません。")
            }
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-nwdiag-\(UUID().uuidString)", isDirectory: true)
            let input = directory.appendingPathComponent("network.diag")
            let output = directory.appendingPathComponent("network.svg")
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                try Data(schema.utf8).write(to: input)
                defer { try? FileManager.default.removeItem(at: directory) }
                let process = Process(); process.executableURL = python
                process.arguments = [wrapper.path, "-T", "svg", "-o", output.path, input.path]
                // Rendering errors can exceed a pipe buffer; write logs to disk while the process runs.
                let log = directory.appendingPathComponent("renderer.log")
                FileManager.default.createFile(atPath: log.path, contents: nil)
                let logHandle = try FileHandle(forWritingTo: log)
                defer { try? logHandle.close() }
                process.standardOutput = logHandle; process.standardError = logHandle
                try process.run(); process.waitUntilExit()
                let err = (try? String(contentsOf: log, encoding: .utf8)) ?? ""
                guard process.terminationStatus == 0, let svg = try? Data(contentsOf: output), !svg.isEmpty else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: err.isEmpty ? "nwdiag SVG生成に失敗しました。" : err)
                }
                let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory
                guard svg.count <= 4 * 1024 * 1024 else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: "NW図が表示可能なサイズを超えました。構成を分割してください。")
                }
                let artifactDirectory = ProcessInfo.processInfo.environment["MIKOMAI_ARTIFACTS_DIR"].map { URL(fileURLWithPath: $0, isDirectory: true) }
                    ?? appSupport.appendingPathComponent("MikomaiDesktopMac/artifacts", isDirectory: true)
                try FileManager.default.createDirectory(at: artifactDirectory, withIntermediateDirectories: true)
                let artifact = artifactDirectory.appendingPathComponent("network-\(UUID().uuidString).svg")
                try svg.write(to: artifact)
                let dataURL = "data:image/svg+xml;base64,\(svg.base64EncodedString())"
                return NetworkOperationOutput(success: true, stdout: "__PORTABLE_ARTIFACT__![Network Diagram](\(dataURL))\n\nSVGを保存しました: \(artifact.path)", stderr: "")
            } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "nwdiagを実行できませんでした: \(error.localizedDescription)") }
        }
        if ["self_network_ping", "self_network_traceroute", "self_network_test_connection", "self_network_test_net_connection", "self_network_route", "network_get_ip_info", "network_list_serial_ports"].contains(tool) {
            switch tool {
            case "self_network_ping", "self_network_traceroute":
                guard let host = arguments["host"] as? String, !host.isEmpty, host.count <= 255,
                      !host.hasPrefix("-"), host.range(of: "^[A-Za-z0-9._:%-]+$", options: .regularExpression) != nil else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: "ホスト名またはIPアドレスが不正です。")
                }
                let process = Process()
                if tool == "self_network_ping" {
                    let command = PingCommand(
                        host: host,
                        size: arguments["size"] as? Int,
                        count: arguments["count"] as? Int,
                        df: (arguments["dont_fragment"] as? Bool) ?? (arguments["df"] as? Bool)
                    )
                    guard let commandArguments = command.processArguments else {
                        return NetworkOperationOutput(success: false, stdout: "", stderr: "Pingのサイズまたは引数が範囲外です。")
                    }
                    process.executableURL = URL(fileURLWithPath: "/sbin/ping")
                    process.arguments = commandArguments
                } else {
                    process.executableURL = URL(fileURLWithPath: "/usr/sbin/traceroute")
                    process.arguments = ["-w", "2", "-m", "15", host]
                }
                let commandText = ([process.executableURL!.path] + (process.arguments ?? [])).joined(separator: " ")
                let out = Pipe(); let err = Pipe()
                process.standardOutput = out; process.standardError = err
                do {
                    try process.run(); process.waitUntilExit()
                    let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr, command: commandText)
                } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription, command: commandText) }
            case "self_network_test_connection", "self_network_test_net_connection":
                guard let host = arguments["host"] as? String, let rawPort = arguments["port"] as? Int,
                      (1...65535).contains(rawPort) else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: "接続先と有効なportが必要です。")
                }
                let result = testTCP(host: host, port: UInt16(rawPort), timeoutMs: 3000)
                return NetworkOperationOutput(success: result.success, stdout: result.success ? result.message : "", stderr: result.success ? "" : result.message)
            case "self_network_route":
                let result = LocalRoutingUtility.read(scope: arguments["scope"] as? String ?? "default", destination: arguments["destination"] as? String)
                return NetworkOperationOutput(success: result.success, stdout: result.stdout, stderr: result.stderr)
            case "network_get_ip_info":
                return runAgentUtility("/sbin/ifconfig", ["-a"])
            default:
                let ports = SerialPortDetector.listPorts().joined(separator: "\n")
                return NetworkOperationOutput(success: true, stdout: ports.isEmpty ? "シリアルポートは見つかりませんでした。" : ports, stderr: "")
            }
        }
        guard let connection = connections.first(where: {
            $0.id.uuidString == target.id || $0.name == target.hostname || $0.host == target.ip
        }) else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "登録済み機器が見つかりません。")
        }
        guard (connection.connectionType ?? "SSH").lowercased() != "console" else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "コンソール接続はこの読み取りエージェントでは未対応です。")
        }
        let resource = arguments["resource"] as? String ?? ""
        let command: String
        switch tool {
        case "network_show":
            guard let supplied = arguments["command"] as? String else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "show コマンドがありません。")
            }
            command = supplied
        case "fetch_config": command = connection.deviceType.lowercased().contains("yamaha") ? "show config" : "show running-config"
        case "fetch_routing": command = "show ip route"
        case "fetch_arp": command = ARPCommandPolicy.command(for: connection.deviceType)
        case "get_state":
            switch resource {
            case "arp": command = ARPCommandPolicy.command(for: connection.deviceType)
            case "routes": command = "show ip route"
            case "interfaces": command = "show interfaces"
            case "lldp": command = "show lldp neighbors"
            case "mac_table": command = "show mac address-table"
            case "bgp": command = "show ip bgp summary"
            case "ospf": command = "show ip ospf neighbor"
            case "cpu": command = CPUUsagePolicy.command(for: connection.deviceType)
            default: return NetworkOperationOutput(success: false, stdout: "", stderr: "未対応の状態リソースです。")
            }
        default:
            return NetworkOperationOutput(success: false, stdout: "", stderr: "この読み取りツールはSwift transportで許可されていません。")
        }
        let credentials = credentialPersistence.load(for: connection.id)
        let deviceType = DeviceTypeCatalog.canonicalID(for: connection.deviceType)
        let request = NetworkRunnerRequest(
            action: "show",
            host: connection.host,
            username: connection.username,
            password: credentials.password ?? "",
            secret: credentials.enablePassword ?? "",
            deviceType: connection.transportDeviceType(deviceType),
            port: connection.effectivePort,
            commands: [command]
        )
        let result = runNetworkWrapper(request)
        if tool == "get_state", resource == "cpu" {
            guard result.success else { return result }
            guard let usage = CPUUsagePolicy.parse(result.stdout) else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "CPU使用率を機器出力から数値として取得できませんでした。")
            }
            return NetworkOperationOutput(success: true, stdout: "{\"usage\":\(usage)}", stderr: "")
        }
        return result
    }

    private nonisolated static func portableAsset(_ relativePath: String) -> URL? {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let roots = [
            ProcessInfo.processInfo.environment["MIKOMAI_ASSETS_DIR"].map { URL(fileURLWithPath: $0) },
            Bundle.main.resourceURL,
            cwd.appendingPathComponent("mikomai-core/assets"),
            cwd.appendingPathComponent("../mikomai-core/assets").standardizedFileURL
        ].compactMap { $0 }
        return roots.map { $0.appendingPathComponent(relativePath) }.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    private nonisolated static func portablePython() -> URL? {
        // Finder launches the development .app with a different working directory.
        // Resolve its repository venv relative to repo/mikomai-desktop-mac/dist/Mikomai.app.
        let developmentPython = Bundle.main.bundleURL.pathExtension == "app"
            ? Bundle.main.bundleURL.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("venv/bin/python")
            : nil
        let candidates = [
            ProcessInfo.processInfo.environment["MIKOMAI_PYTHON"].map { URL(fileURLWithPath: $0) },
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("venv/bin/python"),
            developmentPython,
            URL(fileURLWithPath: "/usr/bin/python3")
        ].compactMap { $0 }
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }

    private nonisolated static func runJSONPython(script: URL, python: URL, payload: [String: Any]) -> NetworkOperationOutput {
        do {
            let process = Process(); process.executableURL = python; process.arguments = [script.path]
            let input = Pipe(); let output = Pipe(); let error = Pipe()
            process.standardInput = input; process.standardOutput = output; process.standardError = error
            try process.run()
            let data = try JSONSerialization.data(withJSONObject: payload)
            input.fileHandleForWriting.write(data); input.fileHandleForWriting.closeFile()
            let stdout = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: error.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            guard process.terminationStatus == 0,
                  let decoded = try JSONSerialization.jsonObject(with: Data(stdout.utf8)) as? [String: Any] else {
                return NetworkOperationOutput(success: false, stdout: stdout, stderr: stderr.isEmpty ? "Config helperの出力を解析できません。" : stderr)
            }
            let success = decoded["success"] as? Bool ?? false
            let pretty = (try? JSONSerialization.data(withJSONObject: decoded, options: [.prettyPrinted, .sortedKeys])).map { String(decoding: $0, as: UTF8.self) } ?? stdout
            return NetworkOperationOutput(success: success, stdout: success ? pretty : "", stderr: success ? "" : (decoded["error"] as? String ?? pretty))
        } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "Config helperを実行できません: \(error.localizedDescription)") }
    }

    private nonisolated static func runAgentUtility(_ executable: String, _ arguments: [String]) -> NetworkOperationOutput {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        let out = Pipe(); let err = Pipe()
        process.standardOutput = out; process.standardError = err
        do {
            try process.run()
            // Drain routing tables while the process runs, before waiting for exit.
            let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
        } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
    }
}
