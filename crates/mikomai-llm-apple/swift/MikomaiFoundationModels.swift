import Foundation
import FoundationModels
import Darwin

// ABI contract: input strings are borrowed UTF-8, NUL terminated. Handles are
// retained objects, used serially and released exactly once after the last call.
// Response/error strings are strdup allocations, freed only by string_free.
// A failed call returns nil and writes an owned error string when possible.
@available(macOS 26.0, *)
private final class BridgeSession: Sendable {
    let session: LanguageModelSession
    init(instructions: String) {
        session = LanguageModelSession(model: SystemLanguageModel.default, instructions: instructions)
    }
}

// The detached task writes once before signaling; the caller reads only after
// semaphore.wait(). No result is accessed concurrently. This also avoids needing
// the caller's main thread or an async runtime to make progress.
private final class Completion: @unchecked Sendable {
    let done = DispatchSemaphore(value: 0)
    var result: Result<String, Error>?
}

private func setError(_ output: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?, _ message: String) {
    output?.pointee = strdup(message)
}

@_cdecl("mikomai_fm_session_create")
public func sessionCreate(
    _ instructions: UnsafePointer<CChar>?,
    _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    error?.pointee = nil
    guard let instructions, let text = String(validatingCString: instructions) else {
        setError(error, "Invalid UTF-8 system instructions")
        return nil
    }
    guard #available(macOS 26.0, *) else {
        setError(error, "Apple Foundation Models requires macOS 26 or later")
        return nil
    }
    switch SystemLanguageModel.default.availability {
    case .available:
        return Unmanaged.passRetained(BridgeSession(instructions: text)).toOpaque()
    case .unavailable(let reason):
        setError(error, "Apple Foundation Models unavailable: \(reason)")
        return nil
    @unknown default:
        setError(error, "Unknown Apple Foundation Models availability")
        return nil
    }
}

// Supported portable subset: objects, arrays, integers and string enums.
// Index ranges guide generation; relationships and completeness remain Core validation.
// Array count guides are intentionally omitted: zero minima fail on this AFM runtime.
@available(macOS 26.0, *)
private func dynamicSchema(_ value: [String: Any], name: String) throws -> DynamicGenerationSchema {
    func invalid(_ message: String) -> NSError {
        NSError(domain: "Mikomai.StructuredSchema", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
    if let choices = value["anyOf"] as? [[String: Any]], !choices.isEmpty {
        return DynamicGenerationSchema(name: name, anyOf: try choices.enumerated().map { idx, choice in
            try dynamicSchema(choice, name: name + "_choice" + String(idx))
        })
    }
    switch value["type"] as? String {
    case "object":
        guard let properties = value["properties"] as? [String: [String: Any]] else { throw invalid("Object requires properties") }
        let required = Set(value["required"] as? [String] ?? [])
        let fields = try properties.keys.sorted().map { key in
            DynamicGenerationSchema.Property(name: key, schema: try dynamicSchema(properties[key]!, name: name + "_" + key), isOptional: !required.contains(key))
        }
        return DynamicGenerationSchema(name: name, properties: fields)
    case "array":
        guard let items = value["items"] as? [String: Any] else { throw invalid("Array requires items") }
        return DynamicGenerationSchema(arrayOf: try dynamicSchema(items, name: name + "_item"))
    case "null":
        guard #available(macOS 26.4, *) else { throw invalid("Nullable schemas require macOS 26.4 or later") }
        return .null
    case "integer":
        if let lower = value["minimum"] as? Int, let upper = value["maximum"] as? Int {
            guard lower <= upper else { throw invalid("Invalid integer range") }
            return DynamicGenerationSchema(type: Int.self, guides: [.range(lower...upper)])
        }
        return DynamicGenerationSchema(type: Int.self)
    case "string":
        guard let choices = value["enum"] as? [String], !choices.isEmpty else { throw invalid("String requires nonempty enum") }
        return DynamicGenerationSchema(name: name, anyOf: choices)
    default:
        throw invalid("Unsupported structured schema type")
    }
}

@_cdecl("mikomai_fm_session_respond")
public func sessionRespond(
    _ handle: UnsafeMutableRawPointer?,
    _ prompt: UnsafePointer<CChar>?,
    _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutablePointer<CChar>? {
    sessionRespondStructured(handle, prompt, nil, error)
}

@_cdecl("mikomai_fm_session_respond_structured")
public func sessionRespondStructured(
    _ handle: UnsafeMutableRawPointer?,
    _ prompt: UnsafePointer<CChar>?,
    _ schema: UnsafePointer<CChar>?,
    _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutablePointer<CChar>? {
    error?.pointee = nil
    guard #available(macOS 26.0, *), let handle, let prompt,
          let text = String(validatingCString: prompt) else {
        setError(error, "Invalid session, prompt UTF-8, or unsupported macOS version")
        return nil
    }
    let generationSchema: GenerationSchema?
    do {
        if let schema {
            guard let text = String(validatingCString: schema),
                  let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else {
                setError(error, "Invalid structured schema JSON or UTF-8")
                return nil
            }
            generationSchema = try GenerationSchema(root: dynamicSchema(value, name: "MikomaiResult"), dependencies: [])
        } else {
            generationSchema = nil
        }
    } catch let failure {
        setError(error, "Invalid structured schema: \(failure)")
        return nil
    }
    let bridge = Unmanaged<BridgeSession>.fromOpaque(handle).takeUnretainedValue()
    let completion = Completion()
    Task.detached {
        do {
            if let generationSchema {
                let response = try await bridge.session.respond(to: text, schema: generationSchema,
                    options: GenerationOptions(temperature: 0, maximumResponseTokens: 1024))
                completion.result = .success(response.content.jsonString)
            } else {
                let response = try await bridge.session.respond(to: text)
                completion.result = .success(response.content)
            }
        } catch {
            completion.result = .failure(error)
        }
        completion.done.signal()
    }
    completion.done.wait()
    switch completion.result {
    case .success(let response):
        // A C string cannot represent embedded NUL without truncation.
        guard !response.utf8.contains(0) else {
            setError(error, "Apple Foundation Models response contains an interior NUL")
            return nil
        }
        guard let output = strdup(response) else {
            setError(error, "Cannot allocate Apple Foundation Models response")
            return nil
        }
        return output
    case .failure(let failure):
        setError(error, "Apple Foundation Models generation failed: \(failure)")
        return nil
    case nil:
        setError(error, "Apple Foundation Models task completed without a result")
        return nil
    }
}

@_cdecl("mikomai_fm_session_destroy")
public func sessionDestroy(_ handle: UnsafeMutableRawPointer?) {
    if #available(macOS 26.0, *), let handle {
        Unmanaged<BridgeSession>.fromOpaque(handle).release()
    }
}

@_cdecl("mikomai_fm_string_free")
public func stringFree(_ value: UnsafeMutablePointer<CChar>?) {
    free(value)
}
