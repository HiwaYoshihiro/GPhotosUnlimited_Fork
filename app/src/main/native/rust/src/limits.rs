//! Size limits for the payload exchanged with the companion.
//!
//! The companion runs as root and reads files from the module directory; the app
//! side allocates buffers from the sizes it receives over the socket. Both ends
//! check the same limits, so a corrupt length, a truncated transfer or a stray
//! file cannot turn into a multi-gigabyte allocation inside Google Photos.
//!
//! The real `classes.dex` is about 20 KiB and the config is smaller than that,
//! so these leave a wide margin.

/// Largest `classes.dex` accepted.
pub const MAX_DEX_SIZE: u64 = 32 * 1024 * 1024;

/// Largest config file accepted.
pub const MAX_CONFIG_SIZE: u64 = 1024 * 1024;
