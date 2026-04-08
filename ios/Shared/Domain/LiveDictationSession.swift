import Foundation
import Observation

enum LiveDictationPhase: String, Codable, Sendable {
    case idle
    case starting
    case ready
    case recording
    case stopping
    case processing
    case completed
    case failed

    var isActive: Bool {
        switch self {
        case .starting, .ready, .recording, .stopping, .processing:
            return true
        case .idle, .completed, .failed:
            return false
        }
    }
}

struct LiveDictationSnapshot: Codable, Equatable, Sendable {
    var sessionID: UUID?
    var phase: LiveDictationPhase
    var selectedStyleID: UUID?
    var finalText: String
    var errorMessage: String
    var requestedStartAt: Date?
    var requestedStopAt: Date?
    var requestedCancelAt: Date?
    var startedAt: Date?
    var updatedAt: Date

    init(
        sessionID: UUID? = nil,
        phase: LiveDictationPhase = .idle,
        selectedStyleID: UUID? = nil,
        finalText: String = "",
        errorMessage: String = "",
        requestedStartAt: Date? = nil,
        requestedStopAt: Date? = nil,
        requestedCancelAt: Date? = nil,
        startedAt: Date? = nil,
        updatedAt: Date = .now
    ) {
        self.sessionID = sessionID
        self.phase = phase
        self.selectedStyleID = selectedStyleID
        self.finalText = finalText
        self.errorMessage = errorMessage
        self.requestedStartAt = requestedStartAt
        self.requestedStopAt = requestedStopAt
        self.requestedCancelAt = requestedCancelAt
        self.startedAt = startedAt
        self.updatedAt = updatedAt
    }
}

@MainActor
@Observable
final class LiveDictationBridge {
    static let shared = LiveDictationBridge()
    private static let suiteName = "group.com.stelath.glide.app"

    nonisolated static let notificationName = "com.glide.live-dictation-changed"

    @ObservationIgnored
    private let store: AppGroupKeyValueStore

    @ObservationIgnored
    private let snapshotKey = "live_dictation_snapshot"

    private(set) var snapshot = LiveDictationSnapshot()

    private init() {
        store = AppGroupKeyValueStore(appGroupIdentifier: Self.suiteName, fileName: "live-dictation.store")
        reloadFromDisk()
        installObserver()
    }

    func reloadFromDisk() {
        guard let data = store.data(forKey: snapshotKey),
              let decoded = try? JSONDecoder().decode(LiveDictationSnapshot.self, from: data)
        else {
            snapshot = LiveDictationSnapshot()
            return
        }

        snapshot = decoded
    }

    @discardableResult
    func beginSession(selectedStyleID: UUID? = nil) -> LiveDictationSnapshot {
        let newSnapshot = LiveDictationSnapshot(
            sessionID: UUID(),
            phase: .starting,
            selectedStyleID: selectedStyleID,
            startedAt: .now,
            updatedAt: .now
        )
        persist(newSnapshot)
        return newSnapshot
    }

    func markRecording(sessionID: UUID) {
        mutate(sessionID: sessionID) {
            $0.phase = .recording
            $0.finalText = ""
            $0.errorMessage = ""
            $0.requestedStartAt = nil
        }
    }

    func markReady(sessionID: UUID, finalText: String = "", errorMessage: String = "") {
        mutate(sessionID: sessionID) {
            $0.phase = .ready
            $0.finalText = finalText
            $0.errorMessage = errorMessage
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func markStopping(sessionID: UUID) {
        mutate(sessionID: sessionID) {
            $0.phase = .stopping
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func markProcessing(sessionID: UUID) {
        mutate(sessionID: sessionID) {
            $0.phase = .processing
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func complete(sessionID: UUID, finalText: String) {
        mutate(sessionID: sessionID) {
            $0.phase = .completed
            $0.finalText = finalText
            $0.errorMessage = ""
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func fail(sessionID: UUID, errorMessage: String) {
        mutate(sessionID: sessionID) {
            $0.phase = .failed
            $0.finalText = ""
            $0.errorMessage = errorMessage
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func reset() {
        persist(LiveDictationSnapshot())
    }

    func requestStart() {
        guard let sessionID = snapshot.sessionID, snapshot.phase == .ready else { return }

        mutate(sessionID: sessionID) {
            $0.requestedStartAt = .now
            $0.requestedStopAt = nil
            $0.requestedCancelAt = nil
        }
    }

    func requestStop() {
        guard let sessionID = snapshot.sessionID, snapshot.phase == .recording else { return }

        mutate(sessionID: sessionID) {
            $0.requestedStartAt = nil
            $0.requestedStopAt = .now
            $0.requestedCancelAt = nil
        }
    }

    func requestCancel() {
        guard let sessionID = snapshot.sessionID, snapshot.phase.isActive else { return }

        mutate(sessionID: sessionID) {
            $0.requestedCancelAt = .now
            $0.requestedStartAt = nil
            $0.requestedStopAt = nil
        }
    }

    func updateSelectedStyleID(_ styleID: UUID?) {
        guard let sessionID = snapshot.sessionID else { return }

        mutate(sessionID: sessionID) {
            $0.selectedStyleID = styleID
        }
    }

    func consumeFinalText() -> String? {
        guard let sessionID = snapshot.sessionID, !snapshot.finalText.isEmpty else { return nil }

        let text = snapshot.finalText.trimmingCharacters(in: .whitespacesAndNewlines)
        mutate(sessionID: sessionID) {
            $0.finalText = ""
        }
        return text.isEmpty ? nil : text
    }

    private func installObserver() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
        CFNotificationCenterAddObserver(
            center,
            observer,
            { _, observer, _, _, _ in
                guard let observer else { return }
                let bridge = Unmanaged<LiveDictationBridge>.fromOpaque(observer).takeUnretainedValue()
                Task { @MainActor in
                    bridge.reloadFromDisk()
                }
            },
            Self.notificationName as CFString,
            nil,
            .deliverImmediately
        )
    }

    private func mutate(sessionID: UUID, _ body: (inout LiveDictationSnapshot) -> Void) {
        guard snapshot.sessionID == sessionID else { return }
        var updated = snapshot
        body(&updated)
        updated.updatedAt = .now
        persist(updated)
    }

    private func persist(_ newSnapshot: LiveDictationSnapshot) {
        snapshot = newSnapshot
        if let data = try? JSONEncoder().encode(newSnapshot) {
            store.set(data, forKey: snapshotKey)
        } else {
            store.removeObject(forKey: snapshotKey)
        }

        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let name = CFNotificationName(Self.notificationName as CFString)
        CFNotificationCenterPostNotification(center, name, nil, nil, true)
    }
}
