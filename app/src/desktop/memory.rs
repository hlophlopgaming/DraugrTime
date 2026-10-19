pub trait MemoryReclaimer {
    fn reclaim(&self);
}
pub struct SystemReclaimer;
impl MemoryReclaimer for SystemReclaimer {
    fn reclaim(&self) {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        // glibc may retain freed UI allocations in thread arenas after renderer destruction.
        unsafe {
            libc::malloc_trim(0);
        }
    }
}

/// Cap glibc malloc arenas before any thread starts. Every thread that allocates
/// otherwise gets its own arena (up to 8 per core); the tracker, loader, tray,
/// API, D-Bus and renderer threads each keep freed pages resident there, and a
/// burst of work on many threads at once (wake from sleep, display
/// reconfiguration, resize) leaves that memory behind until malloc_trim.
pub fn limit_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: called on the main thread before other threads exist; mallopt
    // only adjusts allocator tuning.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
}
