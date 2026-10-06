import Foundation
import Testing
@testable import MikomaiDesktopCore

// Run against an explicitly isolated database, never the user's application DB.
@Suite(.serialized, .enabled(if: ProcessInfo.processInfo.environment["MIKOMAI_GRAPH_DB_PATH"]?.contains("mikomai-test-") == true))
struct NativePersistenceTests {
    @Test func savesSessionsConnectionsAndSettingsThroughRust() throws {
        let session = ChatSession(title: "日本語の会話\n引用符\"", messages: [ChatMessage(role: .user, text: "VLAN 設定\n\"確認\"")], updatedAt: Date(timeIntervalSinceReferenceDate: 1234))
        let snapshot = ChatSessionState(sessions: [session], activeSessionID: session.id)
        try NativePersistence.save(snapshot, collection: "sessions")
        let restored: ChatSessionState? = try NativePersistence.load("sessions")
        #expect(restored == snapshot)

        let connections = [SavedConnection(name: "router", host: "192.0.2.1")]
        try NativePersistence.save(connections, collection: "connections")
        let restoredConnections: [SavedConnection]? = try NativePersistence.load("connections")
        #expect(restoredConnections == connections)

        let settings = DesktopSettings(temperature: 0.3, modelPath: "/models/test.gguf", documentsDirectory: "/資料", knowledgeDirectory: "/knowledge")
        try NativePersistence.save(settings, collection: "settings")
        let restoredSettings: DesktopSettings? = try NativePersistence.load("settings")
        #expect(restoredSettings == settings)

        #expect(throws: NativeStoreError.self) {
            try NativePersistence.save(["password": "must-not-persist"], collection: "settings")
        }
        let unchanged: DesktopSettings? = try NativePersistence.load("settings")
        #expect(unchanged == settings)
        #expect(throws: NativeStoreError.self) {
            let _: DesktopSettings? = try NativePersistence.load("settings; DELETE sessions")
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["MIKOMAI_TEST_STORAGE_REOPEN"] == "1"))
    func reopensDataSavedByAPreviousProcess() throws {
        let snapshot: ChatSessionState? = try NativePersistence.load("sessions")
        #expect(snapshot?.sessions.first?.title == "日本語の会話\n引用符\"")
        #expect(snapshot?.sessions.first?.messages.first?.text == "VLAN 設定\n\"確認\"")
        #expect(snapshot?.activeSessionID == snapshot?.sessions.first?.id)
        let connections: [SavedConnection]? = try NativePersistence.load("connections")
        #expect(connections?.first?.host == "192.0.2.1")
        let settings: DesktopSettings? = try NativePersistence.load("settings")
        #expect(settings?.documentsDirectory == "/資料")
        #expect(settings?.temperature == 0.3)
    }
}
