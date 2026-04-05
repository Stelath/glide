import Foundation

final class AppGroupKeyValueStore {
    private let fileURL: URL
    private let lock = NSLock()

    init(appGroupIdentifier: String, fileName: String) {
        let baseURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        fileURL = baseURL.appendingPathComponent(fileName)
    }

    func string(forKey key: String) -> String? {
        loadValues()[key] as? String
    }

    func bool(forKey key: String) -> Bool {
        loadValues()[key] as? Bool ?? false
    }

    func data(forKey key: String) -> Data? {
        loadValues()[key] as? Data
    }

    func set(_ value: Any, forKey key: String) {
        mutate { values in
            values[key] = value
        }
    }

    func removeObject(forKey key: String) {
        mutate { values in
            values.removeValue(forKey: key)
        }
    }

    private func mutate(_ body: (inout [String: Any]) -> Void) {
        lock.lock()
        defer { lock.unlock() }

        var values = loadValuesUnlocked()
        body(&values)
        saveValuesUnlocked(values)
    }

    private func loadValues() -> [String: Any] {
        lock.lock()
        defer { lock.unlock() }
        return loadValuesUnlocked()
    }

    private func loadValuesUnlocked() -> [String: Any] {
        guard let data = try? Data(contentsOf: fileURL) else { return [:] }
        guard let plist = try? PropertyListSerialization.propertyList(from: data, options: [], format: nil) else {
            return [:]
        }
        return plist as? [String: Any] ?? [:]
    }

    private func saveValuesUnlocked(_ values: [String: Any]) {
        let directoryURL = fileURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: directoryURL, withIntermediateDirectories: true)

        guard let data = try? PropertyListSerialization.data(fromPropertyList: values, format: .binary, options: 0) else {
            return
        }

        try? data.write(to: fileURL, options: .atomic)
    }
}
