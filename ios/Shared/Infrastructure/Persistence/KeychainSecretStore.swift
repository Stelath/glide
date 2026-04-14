import Foundation
import Security

final class KeychainSecretStore {
    private let accessGroup: String?
    private let fallbackStore: AppGroupKeyValueStore

    init(accessGroup: String?, fallbackStore: AppGroupKeyValueStore) {
        self.accessGroup = accessGroup
        self.fallbackStore = fallbackStore
    }

    func read(service: String, account: String? = nil) -> String {
        let effectiveAccount = account ?? service
        for group in accessGroupCandidates {
            let query = makeQuery(service: service, account: effectiveAccount, accessGroup: group, returnData: true)
            var result: AnyObject?
            let status = SecItemCopyMatching(query as CFDictionary, &result)
            if status == errSecSuccess,
               let data = result as? Data,
               let value = String(data: data, encoding: .utf8) {
                return value
            }
        }
        return fallbackStore.string(forKey: fallbackKey(service: service, account: effectiveAccount)) ?? ""
    }

    func write(service: String, account: String? = nil, value: String) {
        let effectiveAccount = account ?? service
        fallbackStore.removeObject(forKey: fallbackKey(service: service, account: effectiveAccount))

        for group in accessGroupCandidates {
            let baseQuery = makeQuery(service: service, account: effectiveAccount, accessGroup: group, returnData: false)
            SecItemDelete(baseQuery as CFDictionary)

            guard !value.isEmpty else { continue }

            var addQuery = baseQuery
            addQuery[kSecValueData as String] = value.data(using: .utf8)
            let status = SecItemAdd(addQuery as CFDictionary, nil)
            if status == errSecSuccess {
                return
            }
        }

        if !value.isEmpty {
            fallbackStore.set(value, forKey: fallbackKey(service: service, account: effectiveAccount))
        }
    }

    func delete(service: String, account: String? = nil) {
        write(service: service, account: account, value: "")
    }

    private var accessGroupCandidates: [String?] {
        if let accessGroup { return [accessGroup, nil] }
        return [nil]
    }

    private func makeQuery(service: String, account: String, accessGroup: String?, returnData: Bool) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        if returnData {
            query[kSecReturnData as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
        }
        return query
    }

    private func fallbackKey(service: String, account: String) -> String {
        account == service ? "fallback_\(service)" : "fallback_\(service)_\(account)"
    }
}
