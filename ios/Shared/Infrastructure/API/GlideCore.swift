import Foundation

enum GlideCoreError: Error, LocalizedError, Sendable {
    case apiError(String)
    case invalidResponse
    case emptyAudio

    var errorDescription: String? {
        switch self {
        case .apiError(let message):
            return message
        case .invalidResponse:
            return "Invalid response from API"
        case .emptyAudio:
            return "Audio buffer is empty"
        }
    }
}

struct FetchModelsResult: Sendable {
    let stt: [String]
    let llm: [String]
}

actor GlideCoreBridge {
    static let shared = GlideCoreBridge()

    private let session: URLSession

    nonisolated var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0.0"
    }

    private init() {
        // These API calls are simple request/response transactions. Keep the
        // session stateless and explicitly avoid eager HTTP/3 racing, which has
        // been producing noisy QUIC/nw_connection warnings during app startup.
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 60
        config.urlCache = nil
        config.httpCookieStorage = nil
        session = URLSession(configuration: config)
    }

    // MARK: - Transcribe (POST multipart to /audio/transcriptions)

    func transcribe(
        audioData: Data,
        provider: String,
        model: String,
        apiKey: String,
        baseURL: String,
        prompt: String? = nil
    ) async throws -> String {
        guard !audioData.isEmpty else {
            throw GlideCoreError.emptyAudio
        }

        let endpoint = "\(baseURL.trimmingSlash)/audio/transcriptions"
        guard let url = URL(string: endpoint) else {
            throw GlideCoreError.apiError("Invalid URL: \(endpoint)")
        }

        let boundary = UUID().uuidString
        var request = makeRequest(url: url, method: "POST", apiKey: apiKey)
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")

        var body = Data()
        body.appendMultipart(boundary: boundary, name: "model", value: model)
        if let prompt, !prompt.isEmpty {
            body.appendMultipart(boundary: boundary, name: "prompt", value: prompt)
        }
        body.appendMultipart(boundary: boundary, name: "file", filename: "glide.wav", mimeType: "audio/wav", data: audioData)
        body.append("--\(boundary)--\r\n".data(using: .utf8)!)
        request.httpBody = body

        let (data, response) = try await session.data(for: request)
        try checkHTTPResponse(response, data: data)

        let parsed = try JSONDecoder().decode(TranscriptionResponse.self, from: data)
        return parsed.text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Cleanup (POST JSON to /chat/completions)

    func cleanup(
        rawText: String,
        provider: String,
        model: String,
        apiKey: String,
        baseURL: String,
        systemPrompt: String
    ) async throws -> String {
        let endpoint = "\(baseURL.trimmingSlash)/chat/completions"
        guard let url = URL(string: endpoint) else {
            throw GlideCoreError.apiError("Invalid URL: \(endpoint)")
        }

        let requestBody = ChatCompletionRequest(
            model: model,
            messages: [
                ChatMessage(role: "system", content: systemPrompt),
                ChatMessage(role: "user", content: rawText),
            ]
        )

        var request = makeRequest(url: url, method: "POST", apiKey: apiKey)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(requestBody)

        let (data, response) = try await session.data(for: request)
        try checkHTTPResponse(response, data: data)

        let parsed = try JSONDecoder().decode(ChatCompletionResponse.self, from: data)
        let text = parsed.choices.first?.message.content.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return Self.stripThinkTags(text)
    }

    // MARK: - Fetch Models (GET /models)

    func fetchModels(apiKey: String, baseURL: String) async throws -> FetchModelsResult {
        let endpoint = "\(baseURL.trimmingSlash)/models"
        guard let url = URL(string: endpoint) else {
            throw GlideCoreError.apiError("Invalid URL: \(endpoint)")
        }

        let request = makeRequest(url: url, method: "GET", apiKey: apiKey)

        let (data, response) = try await session.data(for: request)
        try checkHTTPResponse(response, data: data)

        let parsed = try JSONDecoder().decode(ModelsResponse.self, from: data)
        return Self.classifyModels(parsed.data)
    }

    // MARK: - HTTP Response Checking

    private func checkHTTPResponse(_ response: URLResponse, data: Data) throws {
        guard let httpResponse = response as? HTTPURLResponse else {
            throw GlideCoreError.invalidResponse
        }
        guard (200...299).contains(httpResponse.statusCode) else {
            // Try to extract error message from JSON response
            if let errorBody = try? JSONDecoder().decode(APIErrorResponse.self, from: data) {
                throw GlideCoreError.apiError("\(httpResponse.statusCode): \(errorBody.error.message)")
            }
            throw GlideCoreError.apiError("HTTP \(httpResponse.statusCode)")
        }
    }

    private func makeRequest(url: URL, method: String, apiKey: String) -> URLRequest {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        request.assumesHTTP3Capable = false
        return request
    }

    // MARK: - Model Classification (ported from Rust)

    private static func classifyModels(_ entries: [ModelEntry]) -> FetchModelsResult {
        var stt: [String] = []
        var llm: [String] = []

        for entry in entries {
            if entry.active == false { continue }

            let idLower = entry.id.lowercased()

            if idLower.contains("whisper") || idLower.contains("distil-whisper") {
                stt.append(entry.id)
                continue
            }

            let excluded = idLower.contains("embedding")
                || idLower.contains("tts")
                || idLower.contains("dall-e")
                || idLower.contains("moderation")
                || idLower.hasPrefix("ft:")
                || idLower.contains("realtime")
                || idLower.contains("-audio-")
                || idLower.contains("davinci")
                || idLower.contains("babbage")
                || idLower.contains("canary")
                || idLower.contains("search")
                || idLower.contains("similarity")
                || idLower.hasPrefix("text-")
                || idLower.hasPrefix("code-")
                || idLower.contains("omni-")
                || idLower.contains("orpheus")

            if !excluded {
                llm.append(entry.id)
            }
        }

        return FetchModelsResult(stt: stt.sorted(), llm: llm.sorted())
    }

    // MARK: - Strip <think> tags (ported from Rust)

    static func stripThinkTags(_ text: String) -> String {
        var result = ""
        var remaining = text[...]

        while let startRange = remaining.range(of: "<think", options: .caseInsensitive) {
            result += remaining[remaining.startIndex..<startRange.lowerBound]
            let afterStart = remaining[startRange.lowerBound...]
            if let endRange = afterStart.range(of: "</think", options: .caseInsensitive) {
                let afterEnd = afterStart[endRange.upperBound...]
                if let closeRange = afterEnd.range(of: ">") {
                    remaining = afterEnd[closeRange.upperBound...]
                } else {
                    remaining = afterStart[afterStart.endIndex...]
                }
            } else {
                remaining = afterStart[afterStart.endIndex...]
            }
        }
        result += remaining
        return result.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

// MARK: - API Types

private struct TranscriptionResponse: Decodable {
    let text: String
}

private struct ChatCompletionRequest: Encodable {
    let model: String
    let messages: [ChatMessage]
}

private struct ChatMessage: Codable {
    let role: String
    let content: String
}

private struct ChatCompletionResponse: Decodable {
    let choices: [ChatChoice]
}

private struct ChatChoice: Decodable {
    let message: ChatMessage
}

private struct ModelsResponse: Decodable {
    let data: [ModelEntry]
}

private struct ModelEntry: Decodable {
    let id: String
    let active: Bool?
}

private struct APIErrorResponse: Decodable {
    let error: APIErrorDetail
}

private struct APIErrorDetail: Decodable {
    let message: String
}

// MARK: - Helpers

private extension String {
    var trimmingSlash: String {
        hasSuffix("/") ? String(dropLast()) : self
    }
}

private extension Data {
    mutating func appendMultipart(boundary: String, name: String, value: String) {
        append("--\(boundary)\r\n".data(using: .utf8)!)
        append("Content-Disposition: form-data; name=\"\(name)\"\r\n\r\n".data(using: .utf8)!)
        append("\(value)\r\n".data(using: .utf8)!)
    }

    mutating func appendMultipart(boundary: String, name: String, filename: String, mimeType: String, data fileData: Data) {
        append("--\(boundary)\r\n".data(using: .utf8)!)
        append("Content-Disposition: form-data; name=\"\(name)\"; filename=\"\(filename)\"\r\n".data(using: .utf8)!)
        append("Content-Type: \(mimeType)\r\n\r\n".data(using: .utf8)!)
        append(fileData)
        append("\r\n".data(using: .utf8)!)
    }
}
