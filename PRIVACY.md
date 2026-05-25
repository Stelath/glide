# Privacy Policy for Glide

**Last updated:** May 25, 2026

This Privacy Policy ("Policy") describes how the Glide application (the "App"), developed by **Ghenti** ("we," "us," or "our"), is designed to handle information when you use it. Ghenti is an informal developer collective and is not a registered legal entity. By installing or using Glide, you acknowledge that you have read and understood this Policy.

This Policy applies only to the Glide desktop application. It does not apply to any third-party services, websites, models, or APIs that you choose to connect Glide to. Those services are governed by their own privacy policies and terms.

---

## 1. Plain-English Summary

- Glide is a local macOS application. It is **not designed** to send your audio, transcripts, or usage information to any server operated by Ghenti.
- Ghenti does **not** operate servers that receive content you dictate through the App. Glide does **not** require or maintain user accounts for use of the App. (Ghenti may operate accounts in connection with other, unrelated products; those accounts are not used by, linked to, or required for Glide.)
- Glide can be configured to use third-party services (such as cloud speech-to-text or language model providers) by you, using your own API key. When you do this, your data is sent **directly from your Mac to that provider**. Ghenti is not a party to that exchange.
- We avoid making absolute, unqualified statements like "we will never collect any data of any kind," because privacy laws in some jurisdictions define terms like "collect," "share," and "sell" very broadly. Instead, we describe below exactly how the App is designed to work so you can make an informed decision.

---

## 2. How the App Is Designed to Handle Information

### 2.1 No Ghenti-Operated Backend

Glide, as currently designed, does not transmit your dictated audio, transcripts, settings, or telemetry to any server controlled or operated by Ghenti. Ghenti does not operate cloud infrastructure that receives content from the App.

### 2.2 On-Device Processing

When you select an on-device provider in Glide's settings (for example, Apple's built-in speech recognition, Apple Foundation Models, or a bundled local model such as Parakeet via sherpa-onnx), audio capture and processing happen on your Mac. Use of Apple's services is subject to your operating system's settings and Apple's privacy practices.

### 2.3 Third-Party Providers You Configure

Glide supports OpenAI-compatible and other third-party APIs. If you configure Glide with credentials for such a provider:

- Audio and/or text is transmitted **directly from your Mac to the endpoint URL you supplied**.
- The third-party provider receives that data and processes it according to **its own terms and privacy policy**, which may include logging, retention, model training, human review, or sharing with sub-processors.
- Ghenti has no visibility into, control over, or responsibility for what that provider does with your data.
- You are solely responsible for selecting providers, reviewing their terms, supplying valid credentials, and any fees or consequences of using them.

### 2.4 Necessary Network Activity

Even when configured for purely on-device use, Glide may make limited outbound network requests for operational reasons. These can include:

- Downloading or updating local model files from their hosting servers (for example, when you first enable a local model).
- Checking for application updates, if such functionality is present.
- DNS, certificate, and other standard operating-system network activity.

Any such requests reveal standard technical information (such as your IP address) to the receiving server, which is the operator of that server — not Ghenti. Ghenti does not host or aggregate this information.

### 2.5 Local Storage on Your Device

The App stores the following on your Mac:

- **Settings and preferences**, in a local configuration file managed by macOS.
- **API keys and credentials** you enter, stored in the **macOS Keychain** using Apple's secure credential storage APIs.
- **Cached model files**, if you have downloaded local models.
- **Operating-system-managed permissions** for microphone, speech recognition, and accessibility.

This data lives on your device and is not transmitted to Ghenti. You can remove it by uninstalling the App, deleting its configuration files, or removing entries from Keychain Access.

---

## 3. Permissions Glide Requests

Glide may request the following macOS permissions, each used solely for the stated purpose:

- **Microphone** — to capture audio for transcription.
- **Speech Recognition** — required by macOS when you choose Apple's on-device speech recognition.
- **Accessibility** — to paste transcribed text into other applications.

You can revoke these permissions at any time in **System Settings → Privacy & Security**.

---

## 4. Third-Party Services and Components

### 4.1 Third-Party Providers

If you choose to send data to a third-party provider through Glide:

- That provider becomes the **controller** (or equivalent) of the data you send.
- You should review that provider's privacy policy and terms before using it.
- Common providers may include, but are not limited to, OpenAI and other OpenAI-compatible endpoints, as well as services exposed through Apple's operating system.

### 4.2 Third-Party Libraries

Glide is built on open-source libraries, frameworks, and models. Those components may, in principle, perform their own network activity or have their own data-handling behavior. Ghenti exercises reasonable care in selecting components but does not warrant the behavior of any third-party code and is not responsible for it.

### 4.3 No Endorsement

Mentioning a service or library in this Policy or in the App is not an endorsement. You use third-party services at your own risk and discretion.

---

## 5. Your Choices and Controls

You can control how the App handles information by:

- Choosing an on-device provider (and avoiding cloud providers entirely).
- Not entering any third-party credentials.
- Revoking macOS permissions for microphone, speech recognition, or accessibility.
- Removing API keys from Keychain Access.
- Deleting locally cached model and configuration files.
- Uninstalling the App.

If you have entered credentials for a third-party provider and want that provider to delete data it received, you must contact the provider directly. Ghenti cannot delete data held by third parties.

---

## 6. Children's Privacy

Glide is not directed to children under the age of 13 (or the equivalent minimum age in your jurisdiction), and Ghenti does not knowingly receive personal information from children, as the App is not designed to transmit information to Ghenti. If you believe a child has used the App in connection with a third-party service, you should contact that service directly.

---

## 7. Regional Notices

The App is designed so that Ghenti generally does not receive personal information from users. We provide the following notices for transparency.

### 7.1 California Residents (CCPA / CPRA)

Under California law, "sale" and "sharing" of personal information are defined broadly. Because the App, as designed, does not transmit your personal information to Ghenti, Ghenti does not sell, share, rent, release, disclose, disseminate, transfer, or otherwise communicate your personal information to third parties for monetary or other valuable consideration through the App. If you transmit information directly to a third-party provider you have configured, that exchange is between you and that provider and is governed by their policies.

### 7.2 European Economic Area, United Kingdom, and Switzerland (GDPR / UK GDPR)

Because Ghenti does not receive personal data through the App as designed, Ghenti does not act as a data controller or processor with respect to data you transmit to providers you configure. Where you use a third-party provider, that provider is the controller of the data you send it.

### 7.3 Other Jurisdictions

If your jurisdiction grants additional rights (such as access, correction, deletion, portability, or objection), those rights generally must be exercised against the entities that actually hold your data — which will typically be the third-party providers you have chosen to use, not Ghenti.

---

## 8. Security

No software or transmission method is 100% secure. While Glide is designed to keep your data local and to store credentials in the macOS Keychain, the overall security of your information depends on:

- The security of your Mac, account, and macOS installation,
- The third-party providers and endpoints you choose to use,
- Your handling of API keys and other credentials, and
- Factors outside Ghenti's control.

You are responsible for keeping your operating system updated and for safeguarding your credentials.

---

## 9. Disclaimers

The App is provided under the GNU General Public License v3 (see the `LICENSE` file), which includes a disclaimer of warranty and a limitation of liability. Those terms apply in full and are incorporated here by reference.

In addition, and without limiting them, Ghenti makes no representations and assumes no liability regarding the conduct, data handling, retention, training practices, security, or breaches of any third-party service or provider that you choose to connect Glide to. Any such exchange is solely between you and that provider.

Where any of the foregoing cannot be given full effect under applicable law, it applies to the maximum extent permitted by law.

---

## 10. Your Responsibilities

By using Glide, you acknowledge and agree that:

- You are responsible for the third-party services and endpoints you configure the App to use.
- You are responsible for reviewing the privacy policies, terms of service, and data-handling practices of those services.
- You are responsible for any API keys, credentials, fees, charges, or other obligations associated with your use of those services.
- You will not use the App to transmit content in violation of applicable law or the terms of any third-party service.
- You are responsible for complying with any laws or regulations that apply to the content you dictate or process (including but not limited to confidentiality obligations, professional privilege, and laws governing recording of conversations).

---

## 11. Changes to This Policy

We may update this Policy from time to time. When we do, we will revise the "Last updated" date at the top. Material changes will, where reasonably practicable, be noted in the App's release notes or repository. Continued use of the App after changes take effect constitutes your acceptance of the updated Policy. If you do not agree with an update, your remedy is to stop using the App.

---

## 12. Severability

If any provision of this Policy is found to be unenforceable or invalid under applicable law, that provision will be modified to the minimum extent necessary to make it enforceable, or severed, and the remaining provisions will continue in full force and effect.

---

## 13. No Legal Advice; Not a Contract

This Policy is a description of how the App is designed to handle information. It is not legal advice and is not a substitute for a contract. Use of the App is also subject to the App's license (see the `LICENSE` file in the source repository).

---

## 14. Contact

Questions about this Policy may be directed to the developers through the issue tracker or contact channels listed in the App's source repository.
