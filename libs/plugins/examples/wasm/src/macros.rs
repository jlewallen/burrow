/// Calls `tracing::info!` macro on the host
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {{
        let string = format!($($arg)+);
        #[allow(unused_unsafe)]
        unsafe { $crate::ffi::console_info(string.as_ptr(), string.len()) };
    }};
}

/// Calls `tracing::warn!` macro on the host
#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {{
        let string = format!($($arg)+);
        #[allow(unused_unsafe)]
        unsafe { $crate::ffi::console_warn(string.as_ptr(), string.len()) };
    }};
}

/// Calls `tracing::error!` macro on the host
#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {{
        let string = format!($($arg)+);
        #[allow(unused_unsafe)]
        unsafe { $crate::ffi::console_error(string.as_ptr(), string.len()) };
    }};
}
