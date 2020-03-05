#[cfg(feature = "enable_logging")]
#[macro_export]
macro_rules! lzma_trace {
    ($($arg:tt)+) => {
        trace!($($arg)+);
    }
}

#[cfg(feature = "enable_logging")]
#[macro_export]
macro_rules! lzma_debug {
    ($($arg:tt)+) => {
        debug!($($arg)+);
    }
}

#[cfg(feature = "enable_logging")]
#[macro_export]
macro_rules! lzma_info {
    ($($arg:tt)+) => {
        info!($($arg)+);
    }
}

#[cfg(not(feature = "enable_logging"))]
#[macro_export]
macro_rules! lzma_trace {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "enable_logging"))]
#[macro_export]
macro_rules! lzma_debug {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "enable_logging"))]
#[macro_export]
macro_rules! lzma_info {
    ($($arg:tt)+) => {};
}
