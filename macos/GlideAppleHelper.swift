import AVFoundation
import Darwin
import Foundation
import FoundationModels
import Security
import Speech

let appleSpeechModelPrefix = "speechanalyzer-"
let appleFoundationDefaultModelId = "apple-foundation-default"

struct HelperResponse: Encodable {
    var ok: Bool
    var text: String?
    var speechModels: [AppleSpeechModelResponse]?
    var appleSpeechAvailable: Bool?
    var appleSpeechReason: String?
    var foundationModels: [AppleFoundationModelResponse]?
    var foundationModelsAvailable: Bool?
    var foundationModelsReason: String?
    var error: String?
}

struct AppleSpeechModelResponse: Encodable {
    var id: String
    var displayName: String
    var localeId: String
    var status: String
    var installed: Bool
    var reserved: Bool
}

struct AppleFoundationModelDef {
    var id: String
    var displayName: String
    var modelName: String
}

struct AppleFoundationModelResponse: Encodable {
    var id: String
    var displayName: String
    var modelName: String
    var available: Bool
    var reason: String
}

struct AppleSpeechInstallEvent: Encodable {
    var ok: Bool
    var event: String
    var modelId: String
    var fractionCompleted: Double?
    var completedUnitCount: Int64?
    var totalUnitCount: Int64?
    var error: String?
}

struct TranscribeRequest: Decodable {
    var audioPath: String
    var modelId: String?
    var vocabulary: [String]?
}

struct SpeechModelRequest: Decodable {
    var modelId: String
}

struct CleanupRequest: Decodable {
    var modelId: String?
    var rawText: String
    var systemPrompt: String
    var targetApp: String?
    var modeHint: String?
}

@main
struct GlideAppleHelper {
    static func main() async {
        guard CommandLine.arguments.count >= 2 else {
            printResponse(.failure("missing helper command"))
            return
        }

        do {
            switch CommandLine.arguments[1] {
            case "capabilities":
                printResponse(capabilities())
            case "speech-models":
                do {
                    let models = try await speechModels()
                    printResponse(
                        HelperResponse(
                            ok: true,
                            speechModels: models,
                            appleSpeechAvailable: true,
                            appleSpeechReason: "available"
                        )
                    )
                } catch {
                    printResponse(
                        HelperResponse(
                            ok: false,
                            appleSpeechAvailable: false,
                            appleSpeechReason: error.localizedDescription,
                            error: error.localizedDescription
                        )
                    )
                }
            case "foundation-models":
                let models = foundationModels()
                printResponse(HelperResponse(ok: true, foundationModels: models))
            case "install-speech-model":
                let request: SpeechModelRequest = try readStdinJSON()
                do {
                    try await installSpeechModel(request)
                } catch {
                    printInstallEvent(
                        AppleSpeechInstallEvent(
                            ok: false,
                            event: "failed",
                            modelId: request.modelId,
                            error: error.localizedDescription
                        )
                    )
                }
            case "release-speech-model":
                let request: SpeechModelRequest = try readStdinJSON()
                try await releaseSpeechModel(request)
                printResponse(HelperResponse(ok: true))
            case "transcribe":
                let request: TranscribeRequest = try readStdinJSON()
                let text = try await transcribe(request)
                printResponse(HelperResponse(ok: true, text: text))
            case "cleanup":
                let request: CleanupRequest = try readStdinJSON()
                let text = try await cleanup(request)
                printResponse(HelperResponse(ok: true, text: text))
            default:
                printResponse(.failure("unknown helper command: \(CommandLine.arguments[1])"))
            }
        } catch {
            printResponse(.failure(error.localizedDescription))
        }
    }

    private static func capabilities() -> HelperResponse {
        let speech = speechAvailability()
        let foundation = foundationAvailability()
        return HelperResponse(
            ok: true,
            appleSpeechAvailable: speech.available,
            appleSpeechReason: speech.reason,
            foundationModels: foundation.models,
            foundationModelsAvailable: foundation.available,
            foundationModelsReason: foundation.reason
        )
    }

    private static func speechModels() async throws -> [AppleSpeechModelResponse] {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Speech locale models require macOS 26 or newer")
        }

        try requireSignedHelper("Apple Speech locale access")

        guard SpeechTranscriber.isAvailable else {
            throw HelperError("SpeechTranscriber is unavailable")
        }

        let auth = await speechAuthorization()
        switch auth {
        case .authorized:
            break
        case .notDetermined:
            throw HelperError("speech recognition permission was not determined")
        case .denied:
            throw HelperError("speech recognition permission denied")
        case .restricted:
            throw HelperError("speech recognition permission restricted")
        @unknown default:
            throw HelperError("unknown speech recognition authorization status")
        }

        let supportedLocales = await SpeechTranscriber.supportedLocales
        guard !supportedLocales.isEmpty else {
            throw HelperError("Apple Speech returned no supported locales")
        }

        let installedLocales = await SpeechTranscriber.installedLocales
        let reservedLocales = await AssetInventory.reservedLocales

        var models: [AppleSpeechModelResponse] = []
        for locale in supportedLocales {
            let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
            let status = await AssetInventory.status(forModules: [transcriber])
            let localeId = locale.identifier
            let displayName = Locale.current.localizedString(forIdentifier: localeId) ?? localeId
            models.append(
                AppleSpeechModelResponse(
                    id: modelId(for: locale),
                    displayName: displayName,
                    localeId: localeId,
                    status: status.description,
                    installed: installedLocales.contains(where: { sameLocale($0, locale) }),
                    reserved: reservedLocales.contains(where: { sameLocale($0, locale) })
                )
            )
        }

        return models.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    private static func installSpeechModel(_ request: SpeechModelRequest) async throws {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Speech locale models require macOS 26 or newer")
        }

        try requireSignedHelper("Apple Speech locale access")

        let locale = try await locale(forModelId: request.modelId)
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)

        if let installer = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            printInstallEvent(
                AppleSpeechInstallEvent(
                    ok: true,
                    event: "progress",
                    modelId: request.modelId,
                    fractionCompleted: installer.progress.fractionCompleted,
                    completedUnitCount: installer.progress.completedUnitCount,
                    totalUnitCount: installer.progress.totalUnitCount
                )
            )

            let observation = installer.progress.observe(
                \.fractionCompleted,
                 options: [.new]
            ) { progress, _ in
                printInstallEvent(
                    AppleSpeechInstallEvent(
                        ok: true,
                        event: "progress",
                        modelId: request.modelId,
                        fractionCompleted: progress.fractionCompleted,
                        completedUnitCount: progress.completedUnitCount,
                        totalUnitCount: progress.totalUnitCount
                    )
                )
            }
            defer {
                observation.invalidate()
            }

            try await installer.downloadAndInstall()
        }

        _ = try await AssetInventory.reserve(locale: locale)
        let status = await AssetInventory.status(forModules: [transcriber])
        guard status == .installed else {
            throw HelperError("Apple Speech model is \(status.description)")
        }

        printInstallEvent(
            AppleSpeechInstallEvent(
                ok: true,
                event: "finished",
                modelId: request.modelId,
                fractionCompleted: 1.0
            )
        )
    }

    private static func releaseSpeechModel(_ request: SpeechModelRequest) async throws {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Speech locale models require macOS 26 or newer")
        }

        try requireSignedHelper("Apple Speech locale access")

        let locale = try await locale(forModelId: request.modelId)
        _ = await AssetInventory.release(reservedLocale: locale)
    }

    private static func transcribe(_ request: TranscribeRequest) async throws -> String {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Speech locale models require macOS 26 or newer")
        }

        try requireSignedHelper("Apple Speech")

        let auth = await speechAuthorization()
        guard auth == .authorized else {
            throw HelperError("Speech recognition permission is not authorized")
        }

        let modelId = request.modelId ?? "\(appleSpeechModelPrefix)\(Locale.current.identifier)"
        let locale = try await locale(forModelId: modelId)
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
        let status = await AssetInventory.status(forModules: [transcriber])
        guard status == .installed else {
            throw HelperError("Apple Speech model \(modelId) is \(status.description)")
        }

        _ = try await AssetInventory.reserve(locale: locale)

        let audioURL = URL(fileURLWithPath: request.audioPath)
        let audioFile = try AVAudioFile(forReading: audioURL)
        let analyzer = SpeechAnalyzer(modules: [transcriber])
        let resultTask = Task {
            var parts: [String] = []
            for try await result in transcriber.results {
                if result.isFinal {
                    let text = String(result.text.characters)
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                    if !text.isEmpty {
                        parts.append(text)
                    }
                }
            }
            return parts.joined(separator: " ")
        }

        try await analyzer.start(inputAudioFile: audioFile, finishAfterFile: true)
        let text = try await resultTask.value
        guard !text.isEmpty else {
            throw HelperError("Apple Speech returned an empty transcript")
        }
        return text
    }

    private static let foundationModelDefs = [
        AppleFoundationModelDef(
            id: appleFoundationDefaultModelId,
            displayName: "Apple Foundation Model",
            modelName: "SystemLanguageModel.default"
        ),
    ]

    private static func foundationModels() -> [AppleFoundationModelResponse] {
        guard #available(macOS 26.0, *) else {
            return foundationModelDefs.map { def in
                foundationModelResponse(def, available: false, reason: "requires macOS 26 or newer")
            }
        }

        let model = SystemLanguageModel.default
        let reason: String
        switch model.availability {
        case .available:
            reason = "available"
        case .unavailable(let unavailableReason):
            reason = "\(unavailableReason)"
        }

        return foundationModelDefs.map { def in
            foundationModelResponse(def, available: model.isAvailable, reason: reason)
        }
    }

    private static func foundationModelResponse(
        _ def: AppleFoundationModelDef,
        available: Bool,
        reason: String
    ) -> AppleFoundationModelResponse {
        AppleFoundationModelResponse(
            id: def.id,
            displayName: def.displayName,
            modelName: def.modelName,
            available: available,
            reason: reason
        )
    }

    private static func foundationDefinition(for modelId: String?) throws -> AppleFoundationModelDef {
        let selected = modelId ?? appleFoundationDefaultModelId
        guard let def = foundationModelDefs.first(where: { $0.id == selected }) else {
            throw HelperError("Unknown Apple Foundation model: \(selected)")
        }
        return def
    }

    private static func foundationLanguageModel(for modelId: String?) throws -> SystemLanguageModel {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Foundation Models require macOS 26 or newer")
        }

        _ = try foundationDefinition(for: modelId)
        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            return model
        case .unavailable(let reason):
            throw HelperError("Apple Foundation Model unavailable: \(reason)")
        }
    }

    private static func cleanup(_ request: CleanupRequest) async throws -> String {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Foundation Models require macOS 26 or newer")
        }

        _ = try foundationLanguageModel(for: request.modelId)

        let session = LanguageModelSession(instructions: request.systemPrompt)
        let response = try await session.respond(
            to: cleanupPrompt(request),
            options: GenerationOptions(
                sampling: .greedy,
                temperature: 0,
                maximumResponseTokens: 512
            )
        )
        return response.content.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func cleanupPrompt(_ request: CleanupRequest) -> String {
        var prompt = ""
        if let targetApp = request.targetApp, !targetApp.isEmpty {
            prompt += "Target app: \(targetApp)\n"
        }
        if let modeHint = request.modeHint, !modeHint.isEmpty {
            prompt += "Writing mode: \(modeHint)\n"
        }
        prompt += "Transcript:\n\(request.rawText)"
        return prompt
    }

    private static func speechAvailability() -> (available: Bool, reason: String) {
        guard #available(macOS 26.0, *) else {
            return (false, "requires macOS 26 or newer")
        }

        guard helperTeamIdentifier() != nil else {
            return (false, "requires a signed app with a team identifier")
        }

        guard SpeechTranscriber.isAvailable else {
            return (false, "SpeechTranscriber is unavailable")
        }

        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized:
            return (true, "available")
        case .notDetermined:
            return (true, "permission not requested")
        case .denied:
            return (false, "permission denied")
        case .restricted:
            return (false, "permission restricted")
        @unknown default:
            return (false, "unknown authorization status")
        }
    }

    private static func foundationAvailability() -> (
        available: Bool,
        reason: String,
        models: [AppleFoundationModelResponse]
    ) {
        let models = foundationModels()
        let availableCount = models.filter(\.available).count
        if availableCount > 0 {
            let label = availableCount == 1 ? "1 available model" : "\(availableCount) available models"
            return (true, label, models)
        }

        let reason = models.first?.reason ?? "unavailable"
        return (false, reason, models)
    }

    private static func locale(forModelId modelId: String) async throws -> Locale {
        guard #available(macOS 26.0, *) else {
            throw HelperError("Apple Speech locale models require macOS 26 or newer")
        }

        guard modelId.hasPrefix(appleSpeechModelPrefix) else {
            throw HelperError("Invalid Apple Speech model id: \(modelId)")
        }

        let requestedId = String(modelId.dropFirst(appleSpeechModelPrefix.count))
        let requested = Locale(identifier: requestedId)
        if let supported = await SpeechTranscriber.supportedLocale(equivalentTo: requested) {
            return supported
        }

        let supportedLocales = await SpeechTranscriber.supportedLocales
        if let exact = supportedLocales.first(where: { normalizedIdentifier($0) == normalizedIdentifier(requested) }) {
            return exact
        }

        throw HelperError("No Apple Speech model found for \(modelId)")
    }

    private static func modelId(for locale: Locale) -> String {
        "\(appleSpeechModelPrefix)\(locale.identifier)"
    }

    private static func sameLocale(_ lhs: Locale, _ rhs: Locale) -> Bool {
        normalizedIdentifier(lhs) == normalizedIdentifier(rhs)
    }

    private static func normalizedIdentifier(_ locale: Locale) -> String {
        locale.identifier.replacingOccurrences(of: "-", with: "_").lowercased()
    }

    private static func speechAuthorization() async -> SFSpeechRecognizerAuthorizationStatus {
        let current = SFSpeechRecognizer.authorizationStatus()
        if current != .notDetermined {
            return current
        }

        return await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { status in
                continuation.resume(returning: status)
            }
        }
    }

    private static func requireSignedHelper(_ feature: String) throws {
        guard helperTeamIdentifier() != nil else {
            throw HelperError("\(feature) requires a signed app with a team identifier")
        }
    }

    private static func helperTeamIdentifier() -> String? {
        var code: SecCode?
        guard SecCodeCopySelf(SecCSFlags(), &code) == errSecSuccess, let code else {
            return nil
        }

        var staticCode: SecStaticCode?
        guard SecCodeCopyStaticCode(code, SecCSFlags(), &staticCode) == errSecSuccess,
              let staticCode
        else {
            return nil
        }

        var info: CFDictionary?
        guard SecCodeCopySigningInformation(
            staticCode,
            SecCSFlags(rawValue: kSecCSSigningInformation),
            &info
        ) == errSecSuccess,
            let dict = info as? [String: Any],
            let teamIdentifier = dict[kSecCodeInfoTeamIdentifier as String] as? String,
            !teamIdentifier.isEmpty
        else {
            return nil
        }

        return teamIdentifier
    }

    private static func readStdinJSON<T: Decodable>() throws -> T {
        let data = FileHandle.standardInput.readDataToEndOfFile()
        return try JSONDecoder().decode(T.self, from: data)
    }

    private static func printResponse(_ response: HelperResponse) {
        do {
            let data = try JSONEncoder().encode(response)
            if let text = String(data: data, encoding: .utf8) {
                print(text)
            }
        } catch {
            print("{\"ok\":false,\"error\":\"failed to encode helper response\"}")
        }
    }

    private static func printInstallEvent(_ event: AppleSpeechInstallEvent) {
        do {
            let data = try JSONEncoder().encode(event)
            if let text = String(data: data, encoding: .utf8) {
                print(text)
                fflush(stdout)
            }
        } catch {
            print("{\"ok\":false,\"event\":\"failed\",\"modelId\":\"unknown\",\"error\":\"failed to encode install event\"}")
            fflush(stdout)
        }
    }
}

struct HelperError: LocalizedError {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    var errorDescription: String? { message }
}

private extension HelperResponse {
    static func failure(_ message: String) -> HelperResponse {
        HelperResponse(ok: false, error: message)
    }
}

@available(macOS 26.0, *)
private extension AssetInventory.Status {
    var description: String {
        switch self {
        case .unsupported:
            return "unsupported"
        case .supported:
            return "supported"
        case .downloading:
            return "downloading"
        case .installed:
            return "installed"
        @unknown default:
            return "unknown"
        }
    }
}
