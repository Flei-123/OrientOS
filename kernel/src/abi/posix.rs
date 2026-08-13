//! Bruecke zwischen Kernel und der POSIX-Kompatibilitaetsschicht.
//!
//! **Diese Datei ist der EINZIGE Verweis des Kernels auf POSIX** — und sie
//! haengt komplett am Cargo-Feature `posix`. Entfernen der Schicht heisst:
//! diese Datei loeschen, in `abi/mod.rs` zwei Zeilen streichen, in
//! `kernel/Cargo.toml` Feature und Dependency streichen. Danach existiert im
//! Baum kein `errno`, kein `fd` und kein `open` mehr.
//!
//! Gegenprobe im Testlauf: `cargo build --no-default-features`.

use alloc::vec::Vec;
use karst_abi_native::{Handle, Result as NResult, Syscall};
use karst_abi_posix::{Fd, NativeCalls};

/// Ein POSIX-Prozesskontext: die Fd-Tabelle ist eine reine Altlast dieser
/// Schicht und existiert im Kernel-Core nicht.
pub struct PosixContext {
    fds: Vec<Option<Handle>>,
}

impl PosixContext {
    /// Leerer Kontext.
    pub fn new() -> Self {
        PosixContext { fds: Vec::new() }
    }
}

impl Default for PosixContext {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeCalls for PosixContext {
    fn call(&mut self, nr: Syscall, args: [u64; 6]) -> NResult<u64> {
        let raw = super::native::dispatch(nr as u64, args);
        if raw < 0 {
            // Rueckuebersetzung der Fehlercodes der nativen ABI.
            Err(match raw {
                -1 => karst_abi_native::Error::BadHandle,
                -2 => karst_abi_native::Error::RightsDenied,
                -3 => karst_abi_native::Error::NotFound,
                -4 => karst_abi_native::Error::InvalidArgs,
                -5 => karst_abi_native::Error::Exhausted,
                -6 => karst_abi_native::Error::WrongType,
                -7 => karst_abi_native::Error::WouldBlock,
                -8 => karst_abi_native::Error::Closed,
                _ => karst_abi_native::Error::NotSupported,
            })
        } else {
            Ok(raw as u64)
        }
    }

    fn handle_for_fd(&self, fd: Fd) -> Option<Handle> {
        self.fds.get(fd as usize).copied().flatten()
    }

    fn install_fd(&mut self, h: Handle) -> Option<Fd> {
        // "kleinste freie Zahl" — genau die POSIX-Regel, die der Core nicht kennt.
        if let Some(i) = self.fds.iter().position(|s| s.is_none()) {
            self.fds[i] = Some(h);
            return Some(i as Fd);
        }
        self.fds.push(Some(h));
        Some((self.fds.len() - 1) as Fd)
    }

    fn remove_fd(&mut self, fd: Fd) -> Option<Handle> {
        let slot = self.fds.get_mut(fd as usize)?;
        slot.take()
    }
}

/// Zeigt im Boot-Log, dass die Uebersetzung wirklich laeuft:
/// `read(2)` auf einen unbekannten Fd muss `-EBADF` liefern.
pub fn self_test() -> i64 {
    let mut ctx = PosixContext::new();
    karst_abi_posix::sys_read(&mut ctx, 7, 0, 0)
}
