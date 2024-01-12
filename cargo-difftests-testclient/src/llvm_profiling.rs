#[cfg(cargo_difftests)]
extern "C" {
    pub static __llvm_profile_runtime: i32;
    pub fn __llvm_profile_set_filename(filename: *const libc::c_char);
    pub fn __llvm_profile_write_file() -> libc::c_int;
    pub fn __llvm_profile_reset_counters();
}

// put dummies for docs.rs
#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_set_filename(_: *const libc::c_char) {}

#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_write_file() -> libc::c_int {
    0
}

#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_reset_counters() {}
