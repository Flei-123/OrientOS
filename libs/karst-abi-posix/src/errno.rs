//! POSIX-`errno`-Werte. **Diese Datei ist der einzige Ort im ganzen Baum, an dem
//! `errno` existiert.** Der Kernel-Core kennt nur [`karst_abi_native::Error`].
//!
//! Die Zahlenwerte folgen der Linux/x86_64-Belegung, damit portierte Programme
//! ohne Uebersetzungstabelle laufen. Das ist eine bewusste Altlast **dieser
//! Schicht** — sie faellt mit ihr ersatzlos weg.

/// Fehlernummern der POSIX-Schicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Errno {
    /// Operation not permitted.
    EPERM = 1,
    /// No such file or directory.
    ENOENT = 2,
    /// Interrupted system call.
    EINTR = 4,
    /// I/O error.
    EIO = 5,
    /// Bad file descriptor.
    EBADF = 9,
    /// Try again / would block.
    EAGAIN = 11,
    /// Out of memory.
    ENOMEM = 12,
    /// Permission denied.
    EACCES = 13,
    /// Device or resource busy.
    EBUSY = 16,
    /// File exists.
    EEXIST = 17,
    /// Not a directory.
    ENOTDIR = 20,
    /// Is a directory.
    EISDIR = 21,
    /// Invalid argument.
    EINVAL = 22,
    /// Too many open files (Prozessgrenze).
    EMFILE = 24,
    /// Inappropriate ioctl for device.
    ENOTTY = 25,
    /// Broken pipe.
    EPIPE = 32,
    /// Function not implemented.
    ENOSYS = 38,
}

impl Errno {
    /// Rohwert fuer die `-errno`-Rueckgabekonvention.
    #[inline]
    pub const fn as_raw(self) -> i32 {
        self as i32
    }

    /// Kurzname, z. B. fuer Diagnoseausgaben.
    pub const fn name(self) -> &'static str {
        match self {
            Errno::EPERM => "EPERM",
            Errno::ENOENT => "ENOENT",
            Errno::EINTR => "EINTR",
            Errno::EIO => "EIO",
            Errno::EBADF => "EBADF",
            Errno::EAGAIN => "EAGAIN",
            Errno::ENOMEM => "ENOMEM",
            Errno::EACCES => "EACCES",
            Errno::EBUSY => "EBUSY",
            Errno::EEXIST => "EEXIST",
            Errno::ENOTDIR => "ENOTDIR",
            Errno::EISDIR => "EISDIR",
            Errno::EINVAL => "EINVAL",
            Errno::EMFILE => "EMFILE",
            Errno::ENOTTY => "ENOTTY",
            Errno::EPIPE => "EPIPE",
            Errno::ENOSYS => "ENOSYS",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_match_posix() {
        assert_eq!(Errno::ENOENT.as_raw(), 2);
        assert_eq!(Errno::EBADF.as_raw(), 9);
        assert_eq!(Errno::ENOSYS.as_raw(), 38);
    }
}
