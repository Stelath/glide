@preconcurrency import AVFoundation
import Foundation
import Observation

@Observable
final class AudioRecorder {
    enum State {
        case idle
        case recording
        case processing
    }

    private(set) var state: State = .idle

    /// Current audio level (0.0–1.0), updated during recording.
    private(set) var audioLevel: Float = 0

    @ObservationIgnored
    private let _level = AudioLevelBox()

    @ObservationIgnored
    private let _tapCount = TapCountBox()

    @ObservationIgnored
    private let _lastInputAt = TimestampBox()

    @ObservationIgnored
    private let _engineStartedAt = TimestampBox()

    @ObservationIgnored
    private var levelLink: CADisplayLink?

    @ObservationIgnored
    private var audioEngine: AVAudioEngine?

    @ObservationIgnored
    private let sampleLock = NSLock()

    @ObservationIgnored
    private var snippetSamples: [Float] = []

    @ObservationIgnored
    private var captureEnabled = false

    @ObservationIgnored
    private var inputSampleRate: Double = 0

    @ObservationIgnored
    private var snippetFrameCount: Int = 0

    private let targetSampleRate: Double = 16_000
    private let healthyInputGracePeriod: TimeInterval = 1.5

    /// Whether the audio engine is actually running (may differ from `state` if engine was stopped externally).
    var isEngineRunning: Bool {
        audioEngine?.isRunning ?? false
    }

    /// Whether non-empty PCM frames have recently reached the input tap.
    var isInputHealthy: Bool {
        guard state == .recording, isEngineRunning else { return false }
        if let recentInputAge {
            return recentInputAge < healthyInputGracePeriod
        }

        let engineStartedAt = _engineStartedAt.value
        guard engineStartedAt.timeIntervalSince1970 > 0 else { return false }
        return Date().timeIntervalSince(engineStartedAt) < healthyInputGracePeriod
    }

    var recentInputAge: TimeInterval? {
        let timestamp = _lastInputAt.value
        guard timestamp.timeIntervalSince1970 > 0 else { return nil }
        return Date().timeIntervalSince(timestamp)
    }

    func startRecording() throws {
        guard state == .idle else { return }

        let session = AVAudioSession.sharedInstance()
        try session.setCategory(
            .playAndRecord,
            mode: .default,
            options: [.duckOthers, .mixWithOthers, .defaultToSpeaker, .allowBluetoothHFP]
        )
        try session.setPreferredIOBufferDuration(0.02)
        try session.setActive(true, options: [])

        let engine = AVAudioEngine()
        let inputNode = engine.inputNode

        var workingFormat = inputNode.outputFormat(forBus: 0)
        if workingFormat.channelCount == 0 || workingFormat.sampleRate == 0 {
            workingFormat = inputNode.inputFormat(forBus: 0)
        }

        guard workingFormat.channelCount > 0, workingFormat.sampleRate > 0 else {
            throw AudioRecorderError.noMicrophone
        }

        guard Self.supportsPCMFormat(workingFormat.commonFormat) else {
            throw AudioRecorderError.unsupportedFormat("Unsupported PCM format: \(workingFormat.commonFormat.rawValue)")
        }

        resetSnippetState(inputSampleRate: workingFormat.sampleRate)
        _tapCount.reset()
        _lastInputAt.value = .distantPast
        _engineStartedAt.value = .now
        _level.value = 0
        audioLevel = 0

        inputNode.removeTap(onBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: workingFormat) { [weak self] buffer, _ in
            self?.handleTap(buffer)
        }

        engine.prepare()
        try engine.start()
        audioEngine = engine
        state = .recording
        print("[AudioRecorder] Engine started — format: \(workingFormat), running: \(engine.isRunning)")

        let link = CADisplayLink(target: DisplayLinkTarget { [weak self] in
            guard let self else { return }
            self.audioLevel = self._level.value
        }, selector: #selector(DisplayLinkTarget.tick))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 15, maximum: 30, preferred: 30)
        link.add(to: .main, forMode: .common)
        levelLink = link
    }

    func stopRecording() -> Data {
        guard state == .recording else { return Data() }

        levelLink?.invalidate()
        levelLink = nil

        audioEngine?.inputNode.removeTap(onBus: 0)
        audioEngine?.stop()
        audioEngine = nil
        _engineStartedAt.value = .distantPast
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)

        let (capturedSamples, sampleRate) = drainSnippetSamples()
        audioLevel = 0
        state = .idle

        guard !capturedSamples.isEmpty else {
            return Data()
        }

        return encodeSnippet(samples: capturedSamples, sourceSampleRate: sampleRate)
    }

    func beginSnippet() {
        guard state == .recording else {
            print("[AudioRecorder] beginSnippet skipped — state: \(state)")
            return
        }

        print(
            "[AudioRecorder] beginSnippet — engine running: \(audioEngine?.isRunning ?? false), " +
            "tapCalls so far: \(_tapCount.value), inputHealthy: \(isInputHealthy)"
        )

        _tapCount.reset()
        sampleLock.lock()
        snippetSamples = []
        snippetFrameCount = 0
        captureEnabled = true
        sampleLock.unlock()

        _level.value = 0
        audioLevel = 0
    }

    func captureSnippet() -> Data {
        guard state == .recording else { return Data() }

        let (capturedSamples, sourceSampleRate) = drainSnippetSamples(disableCapture: true)
        let inputAge = recentInputAge.map { String(format: "%.2f", $0) } ?? "n/a"
        print(
            "[AudioRecorder] captureSnippet — \(capturedSamples.count) source samples @ \(Int(sourceSampleRate)) Hz, " +
            "tapCalls: \(_tapCount.value), level: \(_level.value), inputAge: \(inputAge)"
        )

        _level.value = 0
        audioLevel = 0

        guard !capturedSamples.isEmpty else {
            return Data()
        }

        return encodeSnippet(samples: capturedSamples, sourceSampleRate: sourceSampleRate)
    }

    func finishProcessing() {}

    private func handleTap(_ buffer: AVAudioPCMBuffer) {
        _tapCount.increment()

        let monoSamples = Self.extractMonoSamples(from: buffer)
        guard !monoSamples.isEmpty else {
            _level.value = 0
            return
        }

        _lastInputAt.value = .now

        var sumOfSquares: Float = 0
        for sample in monoSamples {
            sumOfSquares += sample * sample
        }

        sampleLock.lock()
        if captureEnabled {
            snippetSamples.append(contentsOf: monoSamples)
            snippetFrameCount += monoSamples.count
        }
        sampleLock.unlock()

        let rms = sqrt(sumOfSquares / Float(monoSamples.count))
        _level.value = min(1.0, rms * 12.0)
    }

    private func resetSnippetState(inputSampleRate: Double) {
        sampleLock.lock()
        snippetSamples = []
        snippetFrameCount = 0
        captureEnabled = false
        self.inputSampleRate = inputSampleRate
        sampleLock.unlock()
    }

    private func drainSnippetSamples(disableCapture: Bool = false) -> ([Float], Double) {
        sampleLock.lock()
        let capturedSamples = snippetSamples
        let sampleRate = inputSampleRate
        snippetSamples = []
        snippetFrameCount = 0
        if disableCapture {
            captureEnabled = false
        }
        sampleLock.unlock()
        return (capturedSamples, sampleRate)
    }

    private func encodeSnippet(samples: [Float], sourceSampleRate: Double) -> Data {
        guard sourceSampleRate > 0 else { return Data() }

        let resampled = Self.resample(samples: samples, from: sourceSampleRate, to: targetSampleRate)
        guard !resampled.isEmpty else { return Data() }

        let pcm16 = resampled.map { sample in
            Int16(max(-1.0, min(1.0, sample)) * 32767.0)
        }
        return encodeWAV(samples: pcm16, sampleRate: UInt32(targetSampleRate))
    }

    private func encodeWAV(samples: [Int16], sampleRate: UInt32) -> Data {
        let channels: UInt16 = 1
        let bitsPerSample: UInt16 = 16
        let byteRate = sampleRate * UInt32(channels) * UInt32(bitsPerSample / 8)
        let blockAlign = channels * (bitsPerSample / 8)
        let dataSize = UInt32(samples.count * MemoryLayout<Int16>.size)
        let fileSize = 36 + dataSize

        var data = Data()
        data.append(contentsOf: "RIFF".utf8)
        data.append(contentsOf: bytes(of: fileSize.littleEndian))
        data.append(contentsOf: "WAVE".utf8)
        data.append(contentsOf: "fmt ".utf8)
        data.append(contentsOf: bytes(of: UInt32(16).littleEndian))
        data.append(contentsOf: bytes(of: UInt16(1).littleEndian))
        data.append(contentsOf: bytes(of: channels.littleEndian))
        data.append(contentsOf: bytes(of: sampleRate.littleEndian))
        data.append(contentsOf: bytes(of: byteRate.littleEndian))
        data.append(contentsOf: bytes(of: blockAlign.littleEndian))
        data.append(contentsOf: bytes(of: bitsPerSample.littleEndian))
        data.append(contentsOf: "data".utf8)
        data.append(contentsOf: bytes(of: dataSize.littleEndian))

        for sample in samples {
            data.append(contentsOf: bytes(of: sample.littleEndian))
        }

        return data
    }

    private func bytes<T>(of value: T) -> [UInt8] {
        withUnsafeBytes(of: value) { Array($0) }
    }

    private static func supportsPCMFormat(_ format: AVAudioCommonFormat) -> Bool {
        switch format {
        case .pcmFormatFloat32, .pcmFormatFloat64, .pcmFormatInt16, .pcmFormatInt32:
            return true
        default:
            return false
        }
    }

    private static func extractMonoSamples(from buffer: AVAudioPCMBuffer) -> [Float] {
        let frameCount = Int(buffer.frameLength)
        let channelCount = Int(buffer.format.channelCount)
        guard frameCount > 0, channelCount > 0 else { return [] }

        switch buffer.format.commonFormat {
        case .pcmFormatFloat32:
            return extractSamples(from: buffer, frameCount: frameCount, channelCount: channelCount) { (pointer: UnsafePointer<Float>) in
                max(-1.0, min(1.0, pointer.pointee))
            }
        case .pcmFormatFloat64:
            return extractSamples(from: buffer, frameCount: frameCount, channelCount: channelCount) { (pointer: UnsafePointer<Double>) in
                Float(max(-1.0, min(1.0, pointer.pointee)))
            }
        case .pcmFormatInt16:
            let scale = Float(Int16.max)
            return extractSamples(from: buffer, frameCount: frameCount, channelCount: channelCount) { (pointer: UnsafePointer<Int16>) in
                Float(pointer.pointee) / scale
            }
        case .pcmFormatInt32:
            let scale = Float(Int32.max)
            return extractSamples(from: buffer, frameCount: frameCount, channelCount: channelCount) { (pointer: UnsafePointer<Int32>) in
                Float(pointer.pointee) / scale
            }
        default:
            return []
        }
    }

    private static func extractSamples<Sample>(
        from buffer: AVAudioPCMBuffer,
        frameCount: Int,
        channelCount: Int,
        normalize: (UnsafePointer<Sample>) -> Float
    ) -> [Float] {
        let audioBuffers = UnsafeMutableAudioBufferListPointer(buffer.mutableAudioBufferList)
        guard !audioBuffers.isEmpty else { return [] }

        var monoSamples = Array(repeating: Float.zero, count: frameCount)

        if buffer.format.isInterleaved {
            guard let rawData = audioBuffers[0].mData else { return [] }
            let pointer = rawData.bindMemory(to: Sample.self, capacity: frameCount * channelCount)

            for frame in 0 ..< frameCount {
                var mixed: Float = 0
                let baseIndex = frame * channelCount
                for channel in 0 ..< channelCount {
                    mixed += normalize(UnsafePointer(pointer.advanced(by: baseIndex + channel)))
                }
                monoSamples[frame] = mixed / Float(channelCount)
            }

            return monoSamples
        }

        let usableChannels = min(channelCount, audioBuffers.count)
        guard usableChannels > 0 else { return [] }

        var channelPointers: [UnsafeMutablePointer<Sample>] = []
        channelPointers.reserveCapacity(usableChannels)
        for channel in 0 ..< usableChannels {
            guard let rawData = audioBuffers[channel].mData else { return [] }
            channelPointers.append(rawData.bindMemory(to: Sample.self, capacity: frameCount))
        }

        for frame in 0 ..< frameCount {
            var mixed: Float = 0
            for channel in 0 ..< usableChannels {
                mixed += normalize(UnsafePointer(channelPointers[channel].advanced(by: frame)))
            }
            monoSamples[frame] = mixed / Float(usableChannels)
        }

        return monoSamples
    }

    private static func resample(samples: [Float], from sourceSampleRate: Double, to targetSampleRate: Double) -> [Float] {
        guard !samples.isEmpty, sourceSampleRate > 0, targetSampleRate > 0 else { return [] }

        if abs(sourceSampleRate - targetSampleRate) < 0.5 {
            return samples
        }

        let targetCount = max(1, Int((Double(samples.count) * targetSampleRate / sourceSampleRate).rounded()))
        guard samples.count > 1 else {
            return Array(repeating: samples[0], count: targetCount)
        }

        var output = Array(repeating: Float.zero, count: targetCount)
        let step = sourceSampleRate / targetSampleRate

        for index in 0 ..< targetCount {
            let sourcePosition = Double(index) * step
            let lowerIndex = min(Int(sourcePosition), samples.count - 1)
            let upperIndex = min(lowerIndex + 1, samples.count - 1)
            let fraction = Float(sourcePosition - Double(lowerIndex))
            let lowerSample = samples[lowerIndex]
            let upperSample = samples[upperIndex]
            output[index] = lowerSample + ((upperSample - lowerSample) * fraction)
        }

        return output
    }
}

enum AudioRecorderError: Error, LocalizedError {
    case setupFailed
    case noMicrophone
    case unsupportedFormat(String)

    var errorDescription: String? {
        switch self {
        case .setupFailed:
            return "Audio setup failed"
        case .noMicrophone:
            return "No microphone found"
        case .unsupportedFormat(let message):
            return message
        }
    }
}

/// Thread-safe box for passing audio level from the tap callback to the main thread.
private final class AudioLevelBox: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: Float = 0

    var value: Float {
        get { lock.lock(); defer { lock.unlock() }; return _value }
        set { lock.lock(); _value = newValue; lock.unlock() }
    }
}

/// Thread-safe counter for diagnosing tap callback delivery.
private final class TapCountBox: @unchecked Sendable {
    private let lock = NSLock()
    private var _count: Int = 0

    func increment() { lock.lock(); _count += 1; lock.unlock() }
    func reset() { lock.lock(); _count = 0; lock.unlock() }
    var value: Int { lock.lock(); defer { lock.unlock() }; return _count }
}

/// Thread-safe timestamp for tracking the last non-empty input buffer.
private final class TimestampBox: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: Date = .distantPast

    var value: Date {
        get { lock.lock(); defer { lock.unlock() }; return _value }
        set { lock.lock(); _value = newValue; lock.unlock() }
    }
}

/// Helper to use CADisplayLink with a closure.
private final class DisplayLinkTarget {
    let callback: () -> Void
    init(_ callback: @escaping () -> Void) { self.callback = callback }
    @objc func tick() { callback() }
}
