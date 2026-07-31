import AppKit
import AuthenticationServices
import Foundation

public let LATTICE_APPLE_SIGNIN_BRIDGE_ABI_VERSION: UInt32 = 1

private enum BridgeCode: Int32 {
    case ok = 0
    case failed = 1
    case cancelled = 2
    case invalidArgument = 3
    case timeout = 4
}

@_cdecl("lattice_apple_signin_bridge_abi_version")
public func lattice_apple_signin_bridge_abi_version() -> UInt32 {
    LATTICE_APPLE_SIGNIN_BRIDGE_ABI_VERSION
}

@_cdecl("lattice_apple_signin_string_free")
public func lattice_apple_signin_string_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}

@_cdecl("lattice_apple_signin_request")
public func lattice_apple_signin_request(
    nonce: UnsafePointer<CChar>?,
    out_token: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    out_error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let out_token else { return BridgeCode.invalidArgument.rawValue }

    let nonceString: String? = nonce.map { String(cString: $0) }.flatMap { value in
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    let box = SignInBox()
    let result = box.run(nonce: nonceString)
    switch result {
    case .success(let token):
        out_token.pointee = strdup(token)
        out_error?.pointee = nil
        return BridgeCode.ok.rawValue
    case .failure(let code, let message):
        out_token.pointee = nil
        if let out_error {
            out_error.pointee = strdup(message)
        }
        return code.rawValue
    }
}

private enum SignInOutcome {
    case success(String)
    case failure(BridgeCode, String)
}

private final class SignInBox: NSObject, ASAuthorizationControllerDelegate,
    ASAuthorizationControllerPresentationContextProviding
{
    private let lock = NSLock()
    private var outcome: SignInOutcome?
    private let semaphore = DispatchSemaphore(value: 0)

    func run(nonce: String?) -> SignInOutcome {
        let work = { [self] in
            let provider = ASAuthorizationAppleIDProvider()
            let request = provider.createRequest()
            request.requestedScopes = [.fullName, .email]
            if let nonce {
                request.nonce = nonce
            }
            let controller = ASAuthorizationController(authorizationRequests: [request])
            controller.delegate = self
            controller.presentationContextProvider = self
            controller.performRequests()
        }

        // ASAuthorizationController delivers UI + delegate callbacks on the main
        // run loop. Blocking main with semaphore.wait freezes the sheet (beachball).
        if Thread.isMainThread {
            work()
            let deadline = Date().addingTimeInterval(180)
            while true {
                lock.lock()
                let current = outcome
                lock.unlock()
                if let current {
                    return current
                }
                if Date() >= deadline {
                    return .failure(.timeout, "Sign in with Apple timed out")
                }
                RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
            }
        }

        DispatchQueue.main.async(execute: work)
        let waitResult = semaphore.wait(timeout: .now() + 180)
        if waitResult == .timedOut {
            return .failure(.timeout, "Sign in with Apple timed out")
        }
        lock.lock()
        defer { lock.unlock() }
        return outcome ?? .failure(.failed, "Sign in with Apple produced no result")
    }

    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        if let key = NSApp.keyWindow {
            return key
        }
        if let main = NSApp.mainWindow {
            return main
        }
        return NSApp.windows.first ?? ASPresentationAnchor()
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
              let tokenData = credential.identityToken,
              let token = String(data: tokenData, encoding: .utf8),
              !token.isEmpty
        else {
            finish(.failure(.failed, "Apple identity token missing"))
            return
        }
        finish(.success(token))
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        let nsError = error as NSError
        if nsError.domain == ASAuthorizationError.errorDomain,
           nsError.code == ASAuthorizationError.canceled.rawValue
        {
            finish(.failure(.cancelled, "Sign in with Apple was cancelled"))
            return
        }
        finish(.failure(.failed, error.localizedDescription))
    }

    private func finish(_ value: SignInOutcome) {
        lock.lock()
        outcome = value
        lock.unlock()
        semaphore.signal()
    }
}
