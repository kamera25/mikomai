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

@_cdecl("mikomai_fm_session_respond")
public func sessionRespond(
    _ handle: UnsafeMutableRawPointer?,
    _ prompt: UnsafePointer<CChar>?,
    _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutablePointer<CChar>? {
    error?.pointee = nil
    guard #available(macOS 26.0, *), let handle, let prompt,
          let text = String(validatingCString: prompt) else {
        setError(error, "Invalid session, prompt UTF-8, or unsupported macOS version")
        return nil
    }
    let bridge = Unmanaged<BridgeSession>.fromOpaque(handle).takeUnretainedValue()
    let completion = Completion()
    Task.detached {
        do {
            let response = try await bridge.session.respond(to: text)
            completion.result = .success(response.content)
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
