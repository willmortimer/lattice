import Foundation

/// Fixed-capacity Float32 ring used for pre-roll while capture is armed.
final class SampleRingBuffer: @unchecked Sendable {
    private var buffer: [Float]
    private let capacity: Int
    private var head: Int = 0
    private var count: Int = 0
    private let lock = NSLock()

    init(capacity: Int) {
        self.capacity = max(capacity, 0)
        self.buffer = [Float](repeating: 0, count: max(capacity, 1))
    }

    var length: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }

    func push(_ samples: UnsafeBufferPointer<Float>) {
        guard capacity > 0, !samples.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }
        for sample in samples {
            if count < capacity {
                let idx = (head + count) % capacity
                buffer[idx] = sample
                count += 1
            } else {
                buffer[head] = sample
                head = (head + 1) % capacity
            }
        }
    }

    func drain() -> [Float] {
        lock.lock()
        defer { lock.unlock() }
        var out = [Float]()
        out.reserveCapacity(count)
        for i in 0..<count {
            out.append(buffer[(head + i) % capacity])
        }
        head = 0
        count = 0
        return out
    }

    func clear() {
        lock.lock()
        defer { lock.unlock() }
        head = 0
        count = 0
    }
}
