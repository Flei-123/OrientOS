//! # karst-abi-posix — ABTRENNBARE POSIX-Kompatibilitaetsschicht
//!
//! **Diese Crate ist ein UEBERSETZER, kein Kernelbestandteil.** Sie darf
//! ausschliesslich von [`karst_abi_native`] abhaengen — niemals von Kernel-
//! internen Strukturen. Umgekehrt darf KEIN Kernel-Core-Modul von hier
//! importieren; der einzige Einsprungpunkt ist [`dispatch`], den der Kernel
//! feature-gated registriert.
//!
//! ## Entfernen der Schicht (siehe ARCHITECTURE.md, Kapitel "POSIX entfernen")
//! 1. `libs/karst-abi-posix/` loeschen,
//! 2. in `kernel/Cargo.toml` Feature `posix` und die optionale Dependency streichen,
//! 3. `kernel/src/abi/mod.rs`: den `#[cfg(feature = "posix")]`-Block streichen.
//! Danach bleibt ein vollstaendiger, lauffaehiger Kernel uebrig — es gibt keinen
//! einzigen weiteren Verweis auf POSIX-Semantik im Baum.
//! Gegenprobe: `cargo build --no-default-features`.
#![no_std]

#[cfg(test)]
extern crate std;

use karst_abi_native::{Error as NErr, Handle, Rights, Syscall};

pub mod errno;

pub use errno::Errno;

/// POSIX-Dateideskriptor. Existiert NUR hier — der Kernel-Core kennt nur
/// [`karst_abi_native::Handle`].
pub type Fd = i32;

/// Die Schnittstelle, die der Kernel dieser Schicht anbietet: ein einziger
/// Einstiegspunkt in die native ABI. Dadurch ist die Schicht komplett
/// entkoppelt und auf dem Host testbar.
pub trait NativeCalls {
    /// Fuehrt einen nativen Syscall aus (Argumente in ABI-Reihenfolge).
    fn call(&mut self, nr: Syscall, args: [u64; 6]) -> karst_abi_native::Result<u64>;

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
fn posix_ret(r: karst_abi_native::Result<u64>) -> i64 {
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

/// `fork(2)` gibt es in Karstos nicht und wird es nie geben: die native ABI
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
        fn call(&mut self, nr: Syscall, args: [u64; 6]) -> karst_abi_native::Result<u64> {
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
}
