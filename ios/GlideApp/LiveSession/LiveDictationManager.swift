import ActivityKit
import AVFoundation
import Observation
import UIKit

private struct LiveDictationProcessingConfig: Sendable {
    let sttProvider: String
    let sttModel: String
    let sttKey: String
    let sttBaseURL: String
    let llmEnabled: Bool
    let llmProvider: String
    let llmModel: String
    let llmKey: String
    let llmBaseURL: String
    let systemPrompt: String
    let vocabularyPrompt: String?
    let replacements: [ReplacementRule]
}

@MainActor
@Observable
final class LiveDictationManager {
    static let shared = LiveDictationManager()

    private(set) var snapshot: LiveDictationSnapshot

    @ObservationIgnored
    private let bridge = LiveDictationBridge.shared

    @ObservationIgnored
    private var recorder = AudioRecorder()

    @ObservationIgnored
    private var processingTask: Task<Void, Never>?

    @ObservationIgnored
    private var processingBackgroundTask: UIBackgroundTaskIdentifier = .invalid

    @ObservationIgnored
    private var currentActivity: Activity<LiveSessionAttributes>?

    @ObservationIgnored
    private var snippetCount: Int = 0

    private init() {
        snapshot = bridge.snapshot
        installObserver()
        installLifecycleObservers()

        if snapshot.phase.isActive {
            bridge.reset()
            snapshot = bridge.snapshot
        }
    }

    /// Preview-only initializer — inert, does not install observers.
    init(preview: Bool) {
        snapshot = LiveDictationSnapshot()
    }

    func startSession(selectedStyleID: UUID? = nil) {
        guard !snapshot.phase.isActive else {
            print("[LiveSession] startSession skipped — already active: \(snapshot.phase)")
            return
        }

        let session = bridge.beginSession(selectedStyleID: selectedStyleID)
        syncSnapshot()

        guard let sessionID = session.sessionID else {
            print("[LiveSession] startSession failed — no sessionID")
            bridge.reset()
            syncSnapshot()
            return
        }

        let micPermission = AVAudioApplication.shared.recordPermission
        print("[LiveSession] mic permission: \(micPermission)")
        guard micPermission == .granted else {
            bridge.fail(sessionID: sessionID, errorMessage: "Allow microphone access in Glide first")
            syncSnapshot()
            return
        }

        do {
            try recorder.startRecording()
            bridge.markReady(sessionID: sessionID)
            syncSnapshot()
            startLiveActivity()
            print("[LiveSession] session started — phase: \(snapshot.phase)")
        } catch {
            print("[LiveSession] startSession error: \(error)")
            bridge.fail(sessionID: sessionID, errorMessage: friendlyErrorMessage(for: error))
            syncSnapshot()
        }
    }

    func stopSession() {
        guard let sessionID = snapshot.sessionID, snapshot.phase == .recording else { return }
        stopAndProcess(sessionID: sessionID)
    }

    func cancelSession() {
        processingTask?.cancel()
        processingTask = nil

        _ = recorder.stopRecording()

        bridge.reset()
        syncSnapshot()
        endProcessingBackgroundTask()
        endLiveActivity()
    }

    private func installObserver() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
        CFNotificationCenterAddObserver(
            center,
            observer,
            { _, observer, _, _, _ in
                guard let observer else { return }
                let manager = Unmanaged<LiveDictationManager>.fromOpaque(observer).takeUnretainedValue()
                Task { @MainActor in
                    manager.handleSharedSessionUpdate()
                }
            },
            LiveDictationBridge.notificationName as CFString,
            nil,
            .deliverImmediately
        )
    }

    private func installLifecycleObservers() {
        let center = NotificationCenter.default
        center.addObserver(
            forName: UIApplication.didEnterBackgroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.handleDidEnterBackground()
            }
        }

        center.addObserver(
            forName: UIApplication.willEnterForegroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.handleWillEnterForeground()
            }
        }

        center.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.handleAudioInterruption()
            }
        }

        center.addObserver(
            forName: AVAudioSession.mediaServicesWereResetNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.recoverRecorderIfNeeded(resetAudioEngine: true)
            }
        }
    }

    private func handleSharedSessionUpdate() {
        syncSnapshot()

        guard snapshot.phase.isActive else { return }

        if snapshot.requestedCancelAt != nil {
            print("[LiveSession] IPC: cancel requested")
            cancelSession()
            return
        }

        if snapshot.requestedStartAt != nil,
           snapshot.phase == .ready,
           let sessionID = snapshot.sessionID {
            print("[LiveSession] IPC: start requested — engine running: \(recorder.isEngineRunning), state: \(recorder.state)")
            beginCapture(sessionID: sessionID)
            return
        }

        if snapshot.requestedStopAt != nil,
           snapshot.phase == .recording,
           let sessionID = snapshot.sessionID {
            print("[LiveSession] IPC: stop requested — engine running: \(recorder.isEngineRunning), state: \(recorder.state)")
            stopAndProcess(sessionID: sessionID)
        }
    }

    private func handleDidEnterBackground() {
        guard snapshot.phase.isActive else { return }
        // The audio background mode (UIBackgroundModes: audio) keeps the session alive
        // automatically. Do NOT create a long-lived UIBackgroundTask here: the app is
        // already entitled to keep recording, and holding the task for the full live
        // session risks the exact watchdog warning we were seeing.
    }

    private func handleWillEnterForeground() {
        guard snapshot.phase.isActive else { return }
        // Recover the recorder if it was stopped while backgrounded
        recoverRecorderIfNeeded(resetAudioEngine: false)
    }

    private func handleAudioInterruption() {
        guard snapshot.phase.isActive else { return }
        recoverRecorderIfNeeded(resetAudioEngine: true)
    }

    private func recoverRecorderIfNeeded(resetAudioEngine: Bool) {
        guard let sessionID = snapshot.sessionID else { return }
        guard snapshot.phase == .ready || snapshot.phase == .recording else { return }

        if resetAudioEngine {
            _ = recorder.stopRecording()
        }

        // Also recover if the engine was silently stopped by iOS (e.g., during background)
        if recorder.state == .recording && !recorder.isEngineRunning {
            _ = recorder.stopRecording()
        }

        if recorder.state == .idle {
            do {
                try recorder.startRecording()
                if snapshot.phase == .recording {
                    bridge.markReady(sessionID: sessionID, errorMessage: "Microphone restarted — tap to record again")
                    updateLiveActivity(phase: "ready")
                } else {
                    bridge.markReady(sessionID: sessionID, finalText: snapshot.finalText, errorMessage: snapshot.errorMessage)
                }
                syncSnapshot()
            } catch {
                bridge.fail(sessionID: sessionID, errorMessage: friendlyErrorMessage(for: error))
                syncSnapshot()
            }
        }
    }

    private func beginCapture(sessionID: UUID) {
        guard snapshot.sessionID == sessionID, snapshot.phase == .ready else { return }

        if recorder.state != .recording || !recorder.isEngineRunning || !recorder.isInputHealthy {
            let inputAge = recorder.recentInputAge.map { String(format: "%.2f", $0) } ?? "n/a"
            print(
                "[LiveSession] beginCapture: recovering — state: \(recorder.state), " +
                "running: \(recorder.isEngineRunning), inputHealthy: \(recorder.isInputHealthy), " +
                "inputAge: \(inputAge)"
            )
            recoverRecorderIfNeeded(resetAudioEngine: true)
            guard recorder.state == .recording, recorder.isEngineRunning, recorder.isInputHealthy else {
                print(
                    "[LiveSession] beginCapture: recovery failed — state: \(recorder.state), " +
                    "running: \(recorder.isEngineRunning), inputHealthy: \(recorder.isInputHealthy)"
                )
                bridge.markReady(sessionID: sessionID, errorMessage: "Microphone lost — tap to try again")
                syncSnapshot()
                return
            }
        }

        recorder.beginSnippet()
        bridge.markRecording(sessionID: sessionID)
        syncSnapshot()
        updateLiveActivity(phase: "recording")
        print("[LiveSession] beginCapture: snippet capture started")
    }

    private func stopAndProcess(sessionID: UUID) {
        guard snapshot.sessionID == sessionID, snapshot.phase == .recording else { return }

        bridge.markStopping(sessionID: sessionID)
        syncSnapshot()

        let audioData = recorder.captureSnippet()
        print("[LiveSession] stopAndProcess: captured \(audioData.count) bytes (header=44, need >44)")

        guard audioData.count > 44 else {
            recoverRecorderForRetry(sessionID: sessionID, errorMessage: "No audio captured")
            return
        }

        bridge.markProcessing(sessionID: sessionID)
        syncSnapshot()
        updateLiveActivity(phase: "processing")

        let processingConfig: LiveDictationProcessingConfig
        do {
            processingConfig = try makeProcessingConfig(for: bridge.snapshot)
        } catch {
            bridge.fail(sessionID: sessionID, errorMessage: friendlyErrorMessage(for: error))
            syncSnapshot()
            return
        }

        beginProcessingBackgroundTask()
        processingTask?.cancel()
        processingTask = Task {
            defer {
                Task { @MainActor in
                    self.endProcessingBackgroundTask()
                }
            }

            do {
                guard audioData.count > 44 else {
                    throw GlideCoreError.emptyAudio
                }

                var text = try await GlideCoreBridge.shared.transcribe(
                    audioData: audioData,
                    provider: processingConfig.sttProvider,
                    model: processingConfig.sttModel,
                    apiKey: processingConfig.sttKey,
                    baseURL: processingConfig.sttBaseURL,
                    prompt: processingConfig.vocabularyPrompt
                )

                text = Self.applyReplacements(text, rules: processingConfig.replacements)

                if processingConfig.llmEnabled,
                   !processingConfig.llmKey.isEmpty,
                   !processingConfig.llmModel.isEmpty {
                    text = try await GlideCoreBridge.shared.cleanup(
                        rawText: text,
                        provider: processingConfig.llmProvider,
                        model: processingConfig.llmModel,
                        apiKey: processingConfig.llmKey,
                        baseURL: processingConfig.llmBaseURL,
                        systemPrompt: processingConfig.systemPrompt
                    )
                }

                let cleanedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if cleanedText.isEmpty || cleanedText == "EMPTY" {
                    throw GlideCoreError.apiError("No transcript returned")
                }

                await MainActor.run {
                    guard self.snapshot.sessionID == sessionID else { return }
                    self.snippetCount += 1
                    self.bridge.markReady(sessionID: sessionID, finalText: cleanedText)
                    self.syncSnapshot()
                    self.updateLiveActivity(phase: "ready")
                }
            } catch is CancellationError {
                await MainActor.run {
                    if self.snapshot.sessionID == sessionID {
                        self.bridge.markReady(sessionID: sessionID)
                        self.syncSnapshot()
                        self.updateLiveActivity(phase: "ready")
                    }
                }
            } catch {
                await MainActor.run {
                    guard self.snapshot.sessionID == sessionID else { return }
                    self.bridge.markReady(sessionID: sessionID, errorMessage: self.friendlyErrorMessage(for: error))
                    self.syncSnapshot()
                    self.updateLiveActivity(phase: "ready")
                }
            }
        }
    }

    private func syncSnapshot() {
        bridge.reloadFromDisk()
        snapshot = bridge.snapshot
    }

    private func makeProcessingConfig(for snapshot: LiveDictationSnapshot) throws -> LiveDictationProcessingConfig {
        let settings = SettingsStore.shared
        let selectedStyle = settings.styles.first(where: { $0.id == snapshot.selectedStyleID })

        let sttProvider = selectedStyle?.sttProvider ?? settings.sttProvider
        let sttModel = selectedStyle?.sttModel ?? settings.sttModel
        let sttKey = settings.apiKey(for: sttProvider)
        let sttBaseURL = settings.baseURL(for: sttProvider)

        guard !sttKey.isEmpty else {
            throw GlideCoreError.apiError("Open Glide to set up your API key")
        }

        let llmProvider = selectedStyle?.llmProvider ?? settings.llmProvider
        let llmModel = selectedStyle?.llmModel ?? settings.llmModel
        let llmKey = settings.apiKey(for: llmProvider)
        let llmBaseURL = settings.baseURL(for: llmProvider)
        let systemPrompt = selectedStyle?.prompt ?? settings.systemPrompt

        let vocab = settings.vocabulary
        let vocabPrompt: String? = vocab.isEmpty ? nil : vocab.joined(separator: ", ")

        return LiveDictationProcessingConfig(
            sttProvider: sttProvider,
            sttModel: sttModel,
            sttKey: sttKey,
            sttBaseURL: sttBaseURL,
            llmEnabled: settings.llmEnabled,
            llmProvider: llmProvider,
            llmModel: llmModel,
            llmKey: llmKey,
            llmBaseURL: llmBaseURL,
            systemPrompt: systemPrompt,
            vocabularyPrompt: vocabPrompt,
            replacements: settings.replacements
        )
    }

    private func beginProcessingBackgroundTask() {
        endProcessingBackgroundTask()

        processingBackgroundTask = UIApplication.shared.beginBackgroundTask(withName: "GlideDictationProcessing") { [weak self] in
            Task { @MainActor in
                self?.cancelSession()
            }
        }
    }

    private func endProcessingBackgroundTask() {
        guard processingBackgroundTask != .invalid else { return }
        UIApplication.shared.endBackgroundTask(processingBackgroundTask)
        processingBackgroundTask = .invalid
    }

    // MARK: - Live Activity

    private func startLiveActivity() {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }

        let attributes = LiveSessionAttributes(startedAt: .now)
        let state = LiveSessionAttributes.ContentState(phase: "ready", snippetCount: 0)
        let content = ActivityContent(state: state, staleDate: Date().addingTimeInterval(8 * 60 * 60))

        do {
            currentActivity = try Activity.request(
                attributes: attributes,
                content: content,
                pushType: nil
            )
        } catch {
            // Live Activity is non-critical — session works without it
        }
    }

    private func updateLiveActivity(phase: String) {
        guard let activity = currentActivity else { return }
        let state = LiveSessionAttributes.ContentState(phase: phase, snippetCount: snippetCount)
        let content = ActivityContent(state: state, staleDate: nil)
        Task {
            await activity.update(content)
        }
    }

    private func endLiveActivity() {
        guard let activity = currentActivity else { return }
        let state = LiveSessionAttributes.ContentState(phase: "ended", snippetCount: snippetCount)
        let content = ActivityContent(state: state, staleDate: nil)
        Task {
            await activity.end(content, dismissalPolicy: .immediate)
        }
        currentActivity = nil
        snippetCount = 0
    }

    private func recoverRecorderForRetry(sessionID: UUID, errorMessage: String) {
        _ = recorder.stopRecording()

        do {
            try recorder.startRecording()
            bridge.markReady(sessionID: sessionID, errorMessage: errorMessage)
            syncSnapshot()
            updateLiveActivity(phase: "ready")
        } catch {
            bridge.fail(sessionID: sessionID, errorMessage: friendlyErrorMessage(for: error))
            syncSnapshot()
            endLiveActivity()
        }
    }

    private static func applyReplacements(_ text: String, rules: [ReplacementRule]) -> String {
        var result = text
        for rule in rules where !rule.find.isEmpty {
            if rule.caseSensitive {
                result = result.replacingOccurrences(of: rule.find, with: rule.replace)
            } else {
                result = result.replacingOccurrences(
                    of: rule.find, with: rule.replace,
                    options: .caseInsensitive
                )
            }
        }
        return result
    }

    private func friendlyErrorMessage(for error: Error) -> String {
        if case GlideCoreError.emptyAudio = error {
            return "No audio captured"
        }

        let message = error.localizedDescription.lowercased()
        if message.contains("401") || message.contains("unauthorized") {
            return "Invalid API key"
        }
        if message.contains("microphone") || message.contains("record") {
            return "Allow microphone access in Glide first"
        }
        if message.contains("empty") || message.contains("no speech") || message.contains("no transcript") {
            return "No audio captured"
        }
        if message.contains("full access") {
            return "Enable Full Access for the keyboard"
        }
        return "Connection failed"
    }
}
