//! # osum-abi-posix — ABTRENNBARE POSIX-Kompatibilitaetsschicht
//!
//! **Diese Crate ist ein UEBERSETZER, kein Kernelbestandteil.** Sie darf
//! ausschliesslich von [`osum_abi_native`] abhaengen — niemals von Kernel-
//! internen Strukturen. Umgekehrt darf KEIN Kernel-Core-Modul von hier
//! importieren; der einzige Einsprungpunkt ist [`dispatch`], den der Kernel
//! feature-gated registriert.
//!
//! ## Entfernen der Schicht (siehe ARCHITECTURE.md, Kapitel "POSIX entfernen")
//! 1. `libs/osum-abi-posix/` loeschen,
//! 2. in `kernel/Cargo.toml` Feature `posix` und die optionale Dependency streichen,
//! 3. `kernel/src/abi/mod.rs`: den `#[cfg(feature = "posix")]`-Block streichen.
//! Danach bleibt ein vollstaendiger, lauffaehiger Kernel uebrig — es gibt keinen
//! einzigen weiteren Verweis auf POSIX-Semantik im Baum.
//! Gegenprobe: `cargo build --no-default-features`.
#![no_std]

#[cfg(test)]
extern crate std;

use osum_abi_native::{Error as NErr, Handle, Rights, Syscall};

pub mod errno;

pub use errno::Errno;

/// POSIX-Dateideskriptor. Existiert NUR hier — der Kernel-Core kennt nur
/// [`osum_abi_native::Handle`].
pub type Fd = i32;

/// Die Schnittstelle, die der Kernel dieser Schicht anbietet: ein einziger
/// Einstiegspunkt in die native ABI. Dadurch ist die Schicht komplett
/// entkoppelt und auf dem Host testbar.
pub trait NativeCalls {
    /// Fuehrt einen nativen Syscall aus (Argumente in ABI-Reihenfolge).
    fn call(&mut self, nr: Syscall, args: [u64; 6]) -> osum_abi_native::Result<u64>;

    /// Bildet einen POSIX-Fd auf ein natives Handle ab.
    fn handle_for_fd(&self, fd: Fd) -> Option<Handle>;

    /// Traegt ein Handle in die Fd-Tabelle ein (Fd-Vergabe: kleinste freie Zahl —
    /// diese POSIX-Altlast lebt bewusst NUR in dieser Crate).
    fn install_fd(&mut self, h: Handle) -> Option<Fd>;

    /// Loescht eine Fd-Zuordnung.
    fn remove_fd(&mut self, fd: Fd) -> Option<Handle>;
}

/// Uebersetzt einen nativen Fehler in einen POSIX-`errno`-Wert.
/// **Nur hier existiert `errno`.**
pub const fn to_errno(e: NErr) -> Errno {
    match e {
        NErr::BadHandle => Errno::EBADF,
        NErr::RightsDenied => Errno::EACCES,
        NErr::NotFound => Errno::ENOENT,
        NErr::InvalidArgs => Errno::EINVAL,
        NErr::Exhausted => Errno::ENOMEM,
        NErr::WrongType => Errno::ENOTTY,
        NErr::WouldBlock => Errno::EAGAIN,
        NErr::Closed => Errno::EPIPE,
        NErr::NotSupported => Errno::ENOSYS,
    }
}

/// POSIX-Rueckgabekonvention: >= 0 Erfolg, sonst `-errno`.
#[inline]
fn posix_ret(r: osum_abi_native::Result<u64>) -> i64 {
    match r {
        Ok(v) => v as i64,
        Err(e) => -(to_errno(e) as i64),
    }
}

/// Exemplarische Uebersetzung: `read(2)`.
///
/// POSIX-Fd -> natives Handle -> [`Syscall::HandleRead`]. Rechtepruefung macht
/// der Kernel anhand der Capability, nicht diese Schicht.
pub fn sys_read<N: NativeCalls>(n: &mut N, fd: Fd, buf: usize, len: usize) -> i64 {
    let Some(h) = n.handle_for_fd(fd) else {
        return -(Errno::EBADF as i64);
    };
    posix_ret(n.call(Syscall::HandleRead, [h.0, buf as u64, len as u64, 0, 0, 0]))
}

/// Exemplarische Uebersetzung: `write(2)`.
pub fn sys_write<N: NativeCalls>(n: &mut N, fd: Fd, buf: usize, len: usize) -> i64 {
    let Some(h) = n.handle_for_fd(fd) else {
        return -(Errno::EBADF as i64);
    };
    posix_ret(n.call(Syscall::HandleWrite, [h.0, buf as u64, len as u64, 0, 0, 0]))
}

/// Exemplarische Uebersetzung: `open(2)`.
///
/// Der POSIX-Pfad wird relativ zum Root-Namespace-Handle des Prozesses
/// aufgeloest — der "globale Root" ist also nur eine Fiktion dieser Schicht.
/// `mode_t` wird bewusst IGNORIERT: Zugriffskontrolle laeuft ueber Capabilities.
pub fn sys_open<N: NativeCalls>(
    n: &mut N,
    root: Handle,
    path_ptr: usize,
    path_len: usize,
    flags: i32,
) -> i64 {
    let rights = oflags_to_rights(flags);
    let r = n.call(
        Syscall::NamespaceOpen,
        [root.0, path_ptr as u64, path_len as u64, rights.bits() as u64, 0, 0],
    );
    match r {
        Ok(raw) => match n.install_fd(Handle(raw)) {
            Some(fd) => fd as i64,
            None => -(Errno::EMFILE as i64),
        },
        Err(e) => -(to_errno(e) as i64),
    }
}

/// Exemplarische Uebersetzung: `close(2)`.
pub fn sys_close<N: NativeCalls>(n: &mut N, fd: Fd) -> i64 {
    match n.remove_fd(fd) {
        Some(h) => posix_ret(n.call(Syscall::HandleClose, [h.0, 0, 0, 0, 0, 0])),
        None => -(Errno::EBADF as i64),
    }
}

/// `fork(2)` gibt es in OrientOS nicht und wird es nie geben: die native ABI
/// kennt kein Adressraum-Kopieren. POSIX-Programme muessen `posix_spawn`
/// benutzen, das auf [`Syscall::ProcessSpawn`] abbildet.
pub const fn sys_fork_unsupported() -> i64 {
    -(Errno::ENOSYS as i64)
}

/// O_RDONLY/O_WRONLY/O_RDWR/O_CREAT -> Capability-Rechte.
pub const O_RDONLY: i32 = 0;
pub const O_WRONLY: i32 = 1;
pub const O_RDWR: i32 = 2;
pub const O_CREAT: i32 = 0o100;

const fn oflags_to_rights(flags: i32) -> Rights {
    let mut r = Rights::INSPECT;
    match flags & 3 {
        O_RDONLY => r = r.union(Rights::READ),
        O_WRONLY => r = r.union(Rights::WRITE),
        O_RDWR => r = r.union(Rights::READ).union(Rights::WRITE),
        _ => {}
    }
    if flags & O_CREAT != 0 {
        r = r.union(Rights::CREATE);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    struct FakeKernel {
        fds: Vec<Option<Handle>>,
        last: Option<(Syscall, [u64; 6])>,
    }

    impl NativeCalls for FakeKernel {
        fn call(&mut self, nr: Syscall, args: [u64; 6]) -> osum_abi_native::Result<u64> {
            self.last = Some((nr, args));
            match nr {
                Syscall::HandleRead | Syscall::HandleWrite => Ok(args[2]),
                Syscall::NamespaceOpen => Ok(Handle::new(3, 1).0),
                Syscall::HandleClose => Ok(0),
                _ => Err(NErr::NotSupported),
            }
        }
        fn handle_for_fd(&self, fd: Fd) -> Option<Handle> {
            self.fds.get(fd as usize).copied().flatten()
        }
        fn install_fd(&mut self, h: Handle) -> Option<Fd> {
            self.fds.push(Some(h));
            Some((self.fds.len() - 1) as Fd)
        }
        fn remove_fd(&mut self, fd: Fd) -> Option<Handle> {
            self.fds.get_mut(fd as usize)?.take()
        }
    }

    fn kernel() -> FakeKernel {
        FakeKernel { fds: Vec::new(), last: None }
    }

    #[test]
    fn open_read_close_translates_to_native() {
        let mut k = kernel();
        let fd = sys_open(&mut k, Handle::new(0, 1), 0x1000, 4, O_RDWR);
        assert!(fd >= 0);
        assert_eq!(k.last.unwrap().0, Syscall::NamespaceOpen);
        assert_eq!(sys_read(&mut k, fd as Fd, 0x2000, 64), 64);
        assert_eq!(k.last.unwrap().0, Syscall::HandleRead);
        assert_eq!(sys_close(&mut k, fd as Fd), 0);
        assert_eq!(sys_read(&mut k, fd as Fd, 0x2000, 64), -(Errno::EBADF as i64));
    }

    #[test]
    fn errors_become_negative_errno() {
        assert_eq!(to_errno(NErr::NotFound), Errno::ENOENT);
        assert!(sys_fork_unsupported() < 0);
    }

    #[test]
    fn oflags_map_to_capabilities() {
        assert!(oflags_to_rights(O_RDONLY).contains(Rights::READ));
        assert!(!oflags_to_rights(O_RDONLY).contains(Rights::WRITE));
        assert!(oflags_to_rights(O_RDWR).contains(Rights::WRITE));
        assert!(oflags_to_rights(O_WRONLY | O_CREAT).contains(Rights::CREATE));
    }

    // ------------------------------------------------------------------------
    // Ein POSIX-getreuerer Nachbau: feste Tabellengroesse, Fd-Vergabe nach
    // "kleinste freie Zahl", und ein einstellbarer Fehler, den die native
    // Schicht zurueckmeldet. Damit lassen sich die Fehlerpfade der Uebersetzung
    // vollstaendig durchspielen, ohne einen Kernel zu starten.
    // ------------------------------------------------------------------------

    struct PosixKernel {
        fds: Vec<Option<Handle>>,
        /// Wenn gesetzt, beantwortet jeder native Aufruf mit diesem Fehler.
        fail_with: Option<NErr>,
        calls: Vec<(Syscall, [u64; 6])>,
        next_slot: u32,
    }

    impl PosixKernel {
        fn with_capacity(cap: usize) -> Self {
            PosixKernel {
                fds: std::vec![None; cap],
                fail_with: None,
                calls: Vec::new(),
                next_slot: 0,
            }
        }
        fn last(&self) -> (Syscall, [u64; 6]) {
            *self.calls.last().expect("kein nativer Aufruf erfolgt")
        }
        fn open_fds(&self) -> usize {
            self.fds.iter().filter(|s| s.is_some()).count()
        }
    }

    impl NativeCalls for PosixKernel {
        fn call(&mut self, nr: Syscall, args: [u64; 6]) -> osum_abi_native::Result<u64> {
            self.calls.push((nr, args));
            if let Some(e) = self.fail_with {
                return Err(e);
            }
            match nr {
                Syscall::HandleRead | Syscall::HandleWrite => Ok(args[2]),
                Syscall::NamespaceOpen => {
                    self.next_slot += 1;
                    Ok(Handle::new(self.next_slot, 1).0)
                }
                Syscall::HandleClose => Ok(0),
                _ => Err(NErr::NotSupported),
            }
        }
        fn handle_for_fd(&self, fd: Fd) -> Option<Handle> {
            if fd < 0 {
                return None;
            }
            self.fds.get(fd as usize).copied().flatten()
        }
        fn install_fd(&mut self, h: Handle) -> Option<Fd> {
            // POSIX-Altlast in Reinform: kleinste freie Zahl gewinnt.
            let i = self.fds.iter().position(|s| s.is_none())?;
            self.fds[i] = Some(h);
            Some(i as Fd)
        }
        fn remove_fd(&mut self, fd: Fd) -> Option<Handle> {
            if fd < 0 {
                return None;
            }
            self.fds.get_mut(fd as usize)?.take()
        }
    }

    /// Alle nativen Fehler — eine neue Variante faellt hier auf.
    const ALL_NATIVE_ERRORS: [NErr; 9] = [
        NErr::BadHandle,
        NErr::RightsDenied,
        NErr::NotFound,
        NErr::InvalidArgs,
        NErr::Exhausted,
        NErr::WrongType,
        NErr::WouldBlock,
        NErr::Closed,
        NErr::NotSupported,
    ];

    #[test]
    fn every_native_error_maps_to_a_distinct_positive_errno() {
        let mut seen = Vec::new();
        for e in ALL_NATIVE_ERRORS {
            let n = to_errno(e);
            assert!(n.as_raw() > 0, "{e:?} -> {n:?} ist nicht positiv");
            assert!(!seen.contains(&n), "{e:?} teilt sich {n:?} mit einem anderen Fehler");
            seen.push(n);
        }
        assert_eq!(seen.len(), ALL_NATIVE_ERRORS.len());
    }

    #[test]
    fn posix_return_convention_is_negative_errno() {
        for e in ALL_NATIVE_ERRORS {
            let r = posix_ret(Err(e));
            assert!(r < 0, "{e:?} kam als Erfolg {r} zurueck");
            assert_eq!(r, -(to_errno(e) as i64));
        }
        for v in [0u64, 1, 4096, i32::MAX as u64] {
            assert_eq!(posix_ret(Ok(v)), v as i64);
        }
    }

    #[test]
    fn read_and_write_pass_handle_buffer_and_length_through() {
        let mut k = PosixKernel::with_capacity(8);
        let fd = sys_open(&mut k, Handle::new(0, 1), 0xdead_000, 5, O_RDWR);
        assert_eq!(fd, 0, "erster Fd muss 0 sein (kleinste freie Zahl)");
        let h = k.handle_for_fd(0).unwrap();

        assert_eq!(sys_write(&mut k, 0, 0xbeef_000, 128), 128);
        let (nr, args) = k.last();
        assert_eq!(nr, Syscall::HandleWrite);
        assert_eq!(args[0], h.0, "falsches Handle im nativen Aufruf");
        assert_eq!(args[1], 0xbeef_000);
        assert_eq!(args[2], 128);
        assert_eq!(&args[3..], &[0, 0, 0], "ungenutzte Argumente muessen 0 sein");

        assert_eq!(sys_read(&mut k, 0, 0x1234_000, 7), 7);
        let (nr, args) = k.last();
        assert_eq!(nr, Syscall::HandleRead);
        assert_eq!(args[0], h.0);
        assert_eq!(args[1], 0x1234_000);
        assert_eq!(args[2], 7);
    }

    #[test]
    fn unknown_fds_never_reach_the_native_layer() {
        let mut k = PosixKernel::with_capacity(4);
        for fd in [0, 3, 99, -1, i32::MIN, i32::MAX] {
            assert_eq!(sys_read(&mut k, fd, 0, 16), -(Errno::EBADF as i64), "fd {fd}");
            assert_eq!(sys_write(&mut k, fd, 0, 16), -(Errno::EBADF as i64), "fd {fd}");
            assert_eq!(sys_close(&mut k, fd), -(Errno::EBADF as i64), "fd {fd}");
        }
        assert!(k.calls.is_empty(), "ungueltiger Fd wurde weitergereicht");
    }

    #[test]
    fn open_uses_the_lowest_free_fd_and_close_frees_it() {
        let mut k = PosixKernel::with_capacity(4);
        let root = Handle::new(0, 1);
        let a = sys_open(&mut k, root, 0, 1, O_RDONLY);
        let b = sys_open(&mut k, root, 0, 1, O_RDONLY);
        let c = sys_open(&mut k, root, 0, 1, O_RDONLY);
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(k.open_fds(), 3);

        assert_eq!(sys_close(&mut k, 1), 0);
        assert_eq!(k.last().0, Syscall::HandleClose);
        assert_eq!(k.open_fds(), 2);
        // Die Luecke muss zuerst wieder vergeben werden — genau diese
        // POSIX-Zusage lebt nur in dieser Schicht.
        assert_eq!(sys_open(&mut k, root, 0, 1, O_RDONLY), 1);
        // Doppeltes close ist EBADF, nicht etwa ein zweiter nativer Aufruf.
        assert_eq!(sys_close(&mut k, 3), -(Errno::EBADF as i64));
    }

    #[test]
    fn full_fd_table_reports_emfile() {
        let mut k = PosixKernel::with_capacity(2);
        let root = Handle::new(0, 1);
        assert_eq!(sys_open(&mut k, root, 0, 1, O_RDONLY), 0);
        assert_eq!(sys_open(&mut k, root, 0, 1, O_RDONLY), 1);
        assert_eq!(sys_open(&mut k, root, 0, 1, O_RDONLY), -(Errno::EMFILE as i64));
        // Nach einem close geht es wieder.
        assert_eq!(sys_close(&mut k, 0), 0);
        assert_eq!(sys_open(&mut k, root, 0, 1, O_RDONLY), 0);
    }

    #[test]
    fn native_errors_are_translated_on_every_call() {
        let root = Handle::new(0, 1);
        for e in ALL_NATIVE_ERRORS {
            let expect = -(to_errno(e) as i64);

            // open: Fehler kommt aus NamespaceOpen.
            let mut k = PosixKernel::with_capacity(4);
            k.fail_with = Some(e);
            assert_eq!(sys_open(&mut k, root, 0, 1, O_RDWR), expect, "open/{e:?}");
            assert_eq!(k.open_fds(), 0, "gescheitertes open darf keinen Fd belegen");

            // read/write/close: Fd existiert, der native Aufruf scheitert.
            let mut k = PosixKernel::with_capacity(4);
            let fd = sys_open(&mut k, root, 0, 1, O_RDWR) as Fd;
            assert_eq!(fd, 0);
            k.fail_with = Some(e);
            assert_eq!(sys_read(&mut k, fd, 0, 8), expect, "read/{e:?}");
            assert_eq!(sys_write(&mut k, fd, 0, 8), expect, "write/{e:?}");
            assert_eq!(sys_close(&mut k, fd), expect, "close/{e:?}");
            // Auch bei Fehler ist der Fd weg — POSIX gibt close() nicht zurueck.
            assert_eq!(k.open_fds(), 0);
        }
    }

    #[test]
    fn open_maps_flags_to_rights_in_the_native_call() {
        let root = Handle::new(7, 2);
        for (flags, needed, forbidden) in [
            (O_RDONLY, Rights::READ, Rights::WRITE),
            (O_WRONLY, Rights::WRITE, Rights::READ),
            (O_RDWR, Rights::READ | Rights::WRITE, Rights::CREATE),
            (O_RDWR | O_CREAT, Rights::CREATE, Rights::EXEC),
        ] {
            let mut k = PosixKernel::with_capacity(4);
            assert!(sys_open(&mut k, root, 0xabc, 3, flags) >= 0);
            let (nr, args) = k.last();
            assert_eq!(nr, Syscall::NamespaceOpen);
            assert_eq!(args[0], root.0, "Pfad wird relativ zum Namespace aufgeloest");
            assert_eq!(args[1], 0xabc);
            assert_eq!(args[2], 3);
            let r = Rights(args[3] as u32);
            assert!(r.contains(needed), "flags {flags:#o}: {needed:?} fehlt");
            assert!(!r.contains(forbidden), "flags {flags:#o}: {forbidden:?} zu viel");
            assert!(r.contains(Rights::INSPECT), "INSPECT gehoert immer dazu");
            assert!(Rights::ALL.contains(r), "unbekanntes Rechtebit erzeugt");
        }
    }

    /// Erschoepfend ueber alle unteren 10 Flag-Bits: die Uebersetzung darf nie
    /// mehr Rechte erzeugen als die drei Zugriffsarten plus CREATE hergeben.
    #[test]
    fn oflags_never_grant_unrelated_rights() {
        for flags in 0i32..1024 {
            let r = oflags_to_rights(flags);
            assert!(Rights::ALL.contains(r), "flags {flags:#o} erzeugt Fremdbits");
            assert!(r.contains(Rights::INSPECT));
            assert!(!r.contains(Rights::EXEC), "flags {flags:#o} vergibt EXEC");
            assert!(!r.contains(Rights::MAP), "flags {flags:#o} vergibt MAP");
            assert!(!r.contains(Rights::TRANSFER));
            assert!(!r.contains(Rights::MANAGE));
            assert_eq!(
                r.contains(Rights::CREATE),
                flags & O_CREAT != 0,
                "CREATE haengt nur an O_CREAT"
            );
            match flags & 3 {
                O_RDONLY => assert!(r.contains(Rights::READ) && !r.contains(Rights::WRITE)),
                O_WRONLY => assert!(r.contains(Rights::WRITE) && !r.contains(Rights::READ)),
                O_RDWR => assert!(r.contains(Rights::READ | Rights::WRITE)),
                // 3 ist kein gueltiger Zugriffsmodus: dann gibt es kein
                // Lese-/Schreibrecht, der Kernel lehnt den Zugriff spaeter ab.
                _ => assert!(!r.contains(Rights::READ) && !r.contains(Rights::WRITE)),
            }
        }
    }

    #[test]
    fn fork_stays_unsupported_and_says_so_in_posix_terms() {
        assert_eq!(sys_fork_unsupported(), -(Errno::ENOSYS as i64));
        assert_eq!(to_errno(NErr::NotSupported), Errno::ENOSYS);
        // Gegenprobe zur Architekturaussage: die native ABI kennt keinen
        // Syscall, der einen Adressraum kopiert.
        for n in 0..=Syscall::MAX {
            let name = Syscall::from_raw(n).unwrap().name();
            assert!(!name.contains("fork"), "{name} widerspricht der Architektur");
        }
    }

    #[test]
    fn the_layer_holds_no_state_of_its_own() {
        // Alle Zustandsfragen (Fd-Tabelle) beantwortet der Kernel ueber das
        // Trait; die Uebersetzungsfunktionen sind reine Funktionen darauf.
        // Zwei unabhaengige "Prozesse" duerfen sich nicht beeinflussen.
        let root = Handle::new(0, 1);
        let mut a = PosixKernel::with_capacity(2);
        let mut b = PosixKernel::with_capacity(2);
        assert_eq!(sys_open(&mut a, root, 0, 1, O_RDONLY), 0);
        assert_eq!(sys_open(&mut a, root, 0, 1, O_RDONLY), 1);
        assert_eq!(sys_open(&mut a, root, 0, 1, O_RDONLY), -(Errno::EMFILE as i64));
        // b hat eine eigene Tabelle und ist davon voellig unberuehrt.
        assert_eq!(sys_open(&mut b, root, 0, 1, O_RDONLY), 0);
        assert_eq!(b.open_fds(), 1);
        assert_eq!(a.open_fds(), 2);
        assert_eq!(sys_read(&mut b, 1, 0, 8), -(Errno::EBADF as i64));
    }
}
