import Foundation

/// Carries a main-actor object into a background scan without pretending the
/// scan itself is isolated.
struct UncheckedSendable<T>: @unchecked Sendable {
    let value: T
}

/// Receives progress from the Rust scan thread.
final class ProgressRelay: ProgressSink, @unchecked Sendable {
    private let handler: @Sendable (ProgressUpdate) -> Void

    init(handler: @escaping @Sendable (ProgressUpdate) -> Void) {
        self.handler = handler
    }

    func onProgress(update: ProgressUpdate) {
        handler(update)
    }
}
