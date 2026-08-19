//! POSIX-`errno`-Werte. **Diese Datei ist der einzige Ort im ganzen Baum, an dem
//! `errno` existiert.** Der Kernel-Core kennt nur [`osum_abi_native::Error`].
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

    /// Alle definierten Fehlernummern mit ihrem Linux/x86_64-Sollwert und dem
    /// erwarteten Kurznamen. Eine neue Variante muss hier eingetragen werden —
    /// sonst schlaegt `table_is_complete_and_sorted` fehl.
    const TABLE: [(Errno, i32, &str); 17] = [
        (Errno::EPERM, 1, "EPERM"),
        (Errno::ENOENT, 2, "ENOENT"),
        (Errno::EINTR, 4, "EINTR"),
        (Errno::EIO, 5, "EIO"),
        (Errno::EBADF, 9, "EBADF"),
        (Errno::EAGAIN, 11, "EAGAIN"),
        (Errno::ENOMEM, 12, "ENOMEM"),
        (Errno::EACCES, 13, "EACCES"),
        (Errno::EBUSY, 16, "EBUSY"),
        (Errno::EEXIST, 17, "EEXIST"),
        (Errno::ENOTDIR, 20, "ENOTDIR"),
        (Errno::EISDIR, 21, "EISDIR"),
        (Errno::EINVAL, 22, "EINVAL"),
        (Errno::EMFILE, 24, "EMFILE"),
        (Errno::ENOTTY, 25, "ENOTTY"),
        (Errno::EPIPE, 32, "EPIPE"),
        (Errno::ENOSYS, 38, "ENOSYS"),
    ];

    #[test]
    fn values_match_posix() {
        assert_eq!(Errno::ENOENT.as_raw(), 2);
        assert_eq!(Errno::EBADF.as_raw(), 9);
        assert_eq!(Errno::ENOSYS.as_raw(), 38);
    }

    #[test]
    fn all_values_match_the_linux_numbering() {
        // Portierte Programme vergleichen gegen diese Zahlen — sie sind Teil
        // der Zusage dieser Schicht und duerfen sich nicht verschieben.
        for (e, soll, _) in TABLE {
            assert_eq!(e.as_raw(), soll, "{e:?} hat die falsche Nummer");
            assert_eq!(e as i32, soll);
        }
    }

    #[test]
    fn values_are_distinct_and_strictly_positive() {
        let mut seen = std::vec::Vec::new();
        for (e, _, _) in TABLE {
            let v = e.as_raw();
            assert!(v > 0, "{e:?} ist nicht positiv — `-errno` waere mehrdeutig");
            assert!(!seen.contains(&v), "Nummer {v} doppelt vergeben");
            seen.push(v);
        }
        assert_eq!(seen.len(), TABLE.len());
    }

    #[test]
    fn names_match_the_variants_and_are_unique() {
        let mut seen = std::vec::Vec::new();
        for (e, _, name) in TABLE {
            assert_eq!(e.name(), name, "Kurzname von {e:?} passt nicht");
            assert!(!name.is_empty() && name.starts_with('E'));
            assert!(
                name.bytes().all(|b| b.is_ascii_uppercase()),
                "{name} nicht in Grossbuchstaben"
            );
            assert!(!seen.contains(&name), "Name {name} doppelt");
            seen.push(name);
        }
    }

    #[test]
    fn table_is_complete_and_sorted() {
        // Streng aufsteigend: haelt die Tabelle lesbar und deckt zugleich auf,
        // wenn zwei Varianten dieselbe Nummer bekommen.
        for w in TABLE.windows(2) {
            assert!(w[0].1 < w[1].1, "{:?} steht vor {:?}", w[0].0, w[1].0);
        }
        assert_eq!(TABLE.len(), 17, "neue Errno-Variante nicht in TABLE eingetragen");
    }

    #[test]
    fn negation_stays_in_the_return_channel() {
        // POSIX-Konvention: -errno im Rueckgabewert. Der Wert muss klar
        // negativ und wieder eindeutig zurueckrechenbar sein.
        for (e, soll, _) in TABLE {
            let ret = -(e as i64);
            assert!(ret < 0);
            assert_eq!(-ret, soll as i64);
            assert!(ret >= -4095, "{e:?} verletzt die uebliche -errno-Schranke");
        }
    }

    #[test]
    fn errno_is_copy_and_comparable() {
        let a = Errno::EAGAIN;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(Errno::EAGAIN, Errno::EBUSY);
    }
}
