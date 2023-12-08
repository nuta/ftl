/// Errors in FTL APIs.
#[derive(Debug)]
pub enum Error {
    HandleNotFound,
    HandleTypeMismatch,
    /// The operation would block. Actually FTL doesn't provide any blocking APIs,
    /// but "would block" is very intuitive for UNIX programmers.
    WouldBlock,
    ClosedByPeer,
    SomethingElse,
}

/// The result type in FTL APIs.
pub type Result<T> = ::core::result::Result<T, crate::Error>;
