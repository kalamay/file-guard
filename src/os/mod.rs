cfg_if::cfg_if! {
    if #[cfg(windows)] {
        pub mod windows;
        pub(crate) use self::windows::{raw_file_lock, raw_file_downgrade};
    } else if #[cfg(unix)] {
        #[macro_use]
        pub mod unix;
        pub(crate) use self::unix::{raw_file_lock, raw_file_downgrade};
    } else {
        // Unknown target_family
    }
}