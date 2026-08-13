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
    /// Ruft der Kernel selbst uebersetzt auf (Selbsttest, Kerneldienste)?
    /// Dann duerfen Puffer im Kernelspeicher liegen. Fuer jeden Aufruf aus
    /// der unprivilegierten Ebene bleibt es bei `false` — dort werden Zeiger
    /// streng geprueft, genau wie bei einem nativen Aufruf.
    kernel_caller: bool,
}

impl PosixContext {
    /// Leerer Kontext fuer einen unprivilegierten Aufrufer.
    pub fn new() -> Self {
        PosixContext { fds: Vec::new(), kernel_caller: false }
    }

    /// Leerer Kontext fuer den Kernel selbst.
    pub fn for_kernel() -> Self {
        PosixContext { fds: Vec::new(), kernel_caller: true }
    }
}

impl Default for PosixContext {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeCalls for PosixContext {
    fn call(&mut self, nr: Syscall, args: [u64; 6]) -> NResult<u64> {
        // Ein Prozess der POSIX-Schicht ist ein gewoehnlicher nativer
        // Prozess: der Aufruf geht durch denselben Verteiler wie jeder andere.
        // Uebersetzt wird nur das ERGEBNIS — Fehlernummern, kein errno im Core.
        let raw = if self.kernel_caller {
            super::native::dispatch_kernel(nr as u64, args)
        } else {
            super::native::dispatch(nr as u64, args)
        };
        karst_abi_native::Error::decode(raw)
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

/// Zeigt im Boot-Log, dass die Uebersetzung wirklich laeuft.
///
/// Durchgespielt wird eine vollstaendige POSIX-Folge auf echten Capabilities:
/// ein Handle wird in die Fd-Tabelle gelegt (das ist die einzige Stelle im
/// Baum, an der es ueberhaupt Fd-Nummern gibt), darauf `write(2)`, dann
/// `close(2)`, danach muss derselbe Fd `-EBADF` liefern. `fork(2)` bleibt
/// `-ENOSYS` — der Kern kennt kein Adressraum-Kopieren.
///
/// Rueckgabe wie bisher: `read(2)` auf einen unbekannten Fd, also `-EBADF`.
pub fn self_test() -> i64 {
    let mut ctx = PosixContext::for_kernel();
    let unbekannt = karst_abi_posix::sys_read(&mut ctx, 7, 0, 0);

    // Eine Capability besorgen und als Fd anmelden. Der Kernel selbst kennt
    // die Nummer nicht — sie lebt ausschliesslich in dieser Schicht.
    let prozess = super::native::current();
    let text = b"POSIX-Uebersetzung auf ein Handle";
    let mut fd = -1i32;
    let mut geschrieben = -1i64;
    let mut nach_close = 0i64;
    if let Ok(h) = super::native::grant_console(
        prozess,
        karst_abi_native::Rights::WRITE | karst_abi_native::Rights::INSPECT,
    ) {
        if let Some(f) = ctx.install_fd(h) {
            fd = f;
            geschrieben =
                karst_abi_posix::sys_write(&mut ctx, f, text.as_ptr() as usize, text.len());
            let _ = karst_abi_posix::sys_close(&mut ctx, f);
            nach_close = karst_abi_posix::sys_write(&mut ctx, f, text.as_ptr() as usize, 1);
        }
    }
    crate::klog!(
        "posix",
        "Uebersetzer: write(fd {}) -> {} B ueber Handle, close(2), danach write -> {} (erwartet {}), fork(2) -> {} (erwartet {})",
        fd,
        geschrieben,
        nach_close,
        -(karst_abi_posix::Errno::EBADF as i64),
        karst_abi_posix::sys_fork_unsupported(),
        -(karst_abi_posix::Errno::ENOSYS as i64)
    );
    unbekannt
}
