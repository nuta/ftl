/// Compile-time assertion.
///
/// # Examples
///
/// ```
/// use essentials::static_assert;
///
/// static_assert!(1 + 1 == 2);
/// static_assert!(1 + 1 == 2, "1 + 1 should be 2");
/// ```
///
/// <https://github.com/rust-lang/rfcs/issues/2790>
/// <https://github.com/rust-lang/rust/pull/89508>
#[macro_export]
macro_rules! static_assert {
    ($cond:expr, $($args:tt)+) => {
        const _: () = ::core::assert!($cond, $($args)+);
    };
    ($cond:expr) => {
        const _: () = ::core::assert!($cond);
    };
}
