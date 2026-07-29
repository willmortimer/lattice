import Foundation

public let LATTICE_APPROVAL_BRIDGE_ABI_VERSION: UInt32 = 1

private enum BridgeCode: Int32 {
    case ok = 0
    case failed = 1
    case notLoaded = 2
    case invalidArgument = 3
}

@_cdecl("lattice_approval_bridge_abi_version")
public func lattice_approval_bridge_abi_version() -> UInt32 {
    LATTICE_APPROVAL_BRIDGE_ABI_VERSION
}

@_cdecl("lattice_approval_load_or_create")
public func lattice_approval_load_or_create() -> Int32 {
    do {
        try SignerRegistry.loadOrCreate()
        return BridgeCode.ok.rawValue
    } catch {
        return BridgeCode.failed.rawValue
    }
}

@_cdecl("lattice_approval_shutdown")
public func lattice_approval_shutdown() {
    SignerRegistry.clear()
}

nonisolated(unsafe) private let backendCString: UnsafePointer<CChar> = UnsafePointer(strdup("secure-enclave")!)

@_cdecl("lattice_approval_backend")
public func lattice_approval_backend() -> UnsafePointer<CChar> {
    backendCString
}

@_cdecl("lattice_approval_device_id")
public func lattice_approval_device_id(
    out: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let out else { return BridgeCode.invalidArgument.rawValue }
    do {
        let signer = try SignerRegistry.current()
        out.pointee = strdup(signer.deviceID)
        return BridgeCode.ok.rawValue
    } catch {
        return BridgeCode.notLoaded.rawValue
    }
}

@_cdecl("lattice_approval_key_id")
public func lattice_approval_key_id(
    out: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let out else { return BridgeCode.invalidArgument.rawValue }
    do {
        let signer = try SignerRegistry.current()
        out.pointee = strdup(signer.keyID)
        return BridgeCode.ok.rawValue
    } catch {
        return BridgeCode.notLoaded.rawValue
    }
}

@_cdecl("lattice_approval_sign")
public func lattice_approval_sign(
    payload: UnsafePointer<UInt8>?,
    payload_len: Int,
    out_sig: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
    out_len: UnsafeMutablePointer<Int>?
) -> Int32 {
    guard let payload, payload_len >= 0, let out_sig, let out_len else {
        return BridgeCode.invalidArgument.rawValue
    }
    do {
        let signer = try SignerRegistry.current()
        let data = Data(bytes: payload, count: payload_len)
        let signature = try signer.sign(payload: data)
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: signature.count)
        signature.copyBytes(to: buffer, count: signature.count)
        out_sig.pointee = buffer
        out_len.pointee = signature.count
        return BridgeCode.ok.rawValue
    } catch {
        return BridgeCode.failed.rawValue
    }
}

@_cdecl("lattice_approval_string_free")
public func lattice_approval_string_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}

@_cdecl("lattice_approval_buffer_free")
public func lattice_approval_buffer_free(_ ptr: UnsafeMutablePointer<UInt8>?, len: Int) {
    guard let ptr, len >= 0 else { return }
    ptr.deallocate()
}
