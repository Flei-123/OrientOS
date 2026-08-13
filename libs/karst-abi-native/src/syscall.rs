//! Syscall-Nummern und -Signaturen von karst-native.
//!
//! Konvention (x86_64, spaeter pro Arch gespiegelt): Nummer in `rax`,
//! Argumente in `rdi, rsi, rdx, r10, r8, r9`, Rueckgabe in `rax` als
//! [`crate::encode`]-Wert. Es gibt KEINE variadischen Syscalls und keine
//! ioctl-artige Sammelschnittstelle.

/// Alle Syscalls der nativen ABI. Nummern sind stabil und werden nur angehaengt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Syscall {
    /// `()` — gibt die ABI-Version zurueck. Dient als Lebenszeichen.
    Version = 0,
    /// `(handle, buf, len) -> gelesene Bytes`. Braucht `Rights::READ`.
    HandleRead = 1,
    /// `(handle, buf, len) -> geschriebene Bytes`. Braucht `Rights::WRITE`.
    HandleWrite = 2,
    /// `(handle) -> ()`. Schliesst ein Handle.
    HandleClose = 3,
    /// `(handle, rights_mask) -> neues Handle`. Braucht `Rights::DUPLICATE`.
    HandleDuplicate = 4,
    /// `(handle, out_info_ptr) -> ()`. Braucht `Rights::INSPECT`.
    HandleInspect = 5,
    /// `(namespace_handle, name_ptr, name_len, rights) -> Handle`.
    /// Ersetzt `open()`: aufgeloest wird relativ zu einem Namespace-Handle,
    /// es gibt keinen prozessglobalen Root.
    NamespaceOpen = 6,
    /// `(namespace_handle, name_ptr, name_len, kind) -> Handle`. Braucht `Rights::CREATE`.
    NamespaceCreate = 7,
    /// `(len, flags) -> Memory-Handle`. Erzeugt ein Speicherobjekt.
    MemoryCreate = 8,
    /// `(memory_handle, vaddr_hint, len, rights) -> vaddr`. Braucht `Rights::MAP`.
    MemoryMap = 9,
    /// `(vaddr, len) -> ()`.
    MemoryUnmap = 10,
    /// `(out_a, out_b) -> ()`. Erzeugt ein verbundenes Channel-Paar.
    ChannelCreate = 11,
    /// `(channel, msg_ptr, msg_len, handles_ptr, handle_count) -> ()`.
    ChannelSend = 12,
    /// `(channel, buf_ptr, buf_len, handles_ptr, handle_cap) -> Nachrichtenlaenge`.
    ChannelRecv = 13,
    /// `() -> Port-Handle`. Ports ersetzen Signals vollstaendig.
    PortCreate = 14,
    /// `(port, packet_out_ptr, deadline_ns) -> ()`. Blockiert bis Ereignis/Deadline.
    PortWait = 15,
    /// `(port, object_handle, key, signals) -> ()`. Objekt an Port binden.
    PortBind = 16,
    /// `(image_handle, namespace_handle, handles_ptr, handle_count) -> Process-Handle`.
    /// Ersetzt `fork`+`exec`: Der Adressraum wird NIE kopiert, Handles werden
    /// explizit aufgelistet — kein implizites Erben.
    ProcessSpawn = 17,
    /// `(process_handle, code) -> ()`.
    ProcessExit = 18,
    /// `(entry, arg, stack_size) -> Thread-Handle`.
    ThreadCreate = 19,
    /// `(deadline_ns) -> ()`.
    ThreadSleep = 20,
    /// `() -> ()`. Gibt die CPU freiwillig ab.
    ThreadYield = 21,
    /// `(clock_id) -> Nanosekunden`.
    ClockRead = 22,
}

impl Syscall {
    /// Hoechste vergebene Nummer (fuer Dispatch-Tabellen).
    pub const MAX: u64 = Syscall::ClockRead as u64;

    /// Umwandlung aus der Rohnummer. `None` = unbekannter Syscall.
    pub const fn from_raw(n: u64) -> Option<Syscall> {
        Some(match n {
            0 => Syscall::Version,
            1 => Syscall::HandleRead,
            2 => Syscall::HandleWrite,
            3 => Syscall::HandleClose,
            4 => Syscall::HandleDuplicate,
            5 => Syscall::HandleInspect,
            6 => Syscall::NamespaceOpen,
            7 => Syscall::NamespaceCreate,
            8 => Syscall::MemoryCreate,
            9 => Syscall::MemoryMap,
            10 => Syscall::MemoryUnmap,
            11 => Syscall::ChannelCreate,
            12 => Syscall::ChannelSend,
            13 => Syscall::ChannelRecv,
            14 => Syscall::PortCreate,
            15 => Syscall::PortWait,
            16 => Syscall::PortBind,
            17 => Syscall::ProcessSpawn,
            18 => Syscall::ProcessExit,
            19 => Syscall::ThreadCreate,
            20 => Syscall::ThreadSleep,
            21 => Syscall::ThreadYield,
            22 => Syscall::ClockRead,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Syscall::Version => "version",
            Syscall::HandleRead => "handle_read",
            Syscall::HandleWrite => "handle_write",
            Syscall::HandleClose => "handle_close",
            Syscall::HandleDuplicate => "handle_duplicate",
            Syscall::HandleInspect => "handle_inspect",
            Syscall::NamespaceOpen => "namespace_open",
            Syscall::NamespaceCreate => "namespace_create",
            Syscall::MemoryCreate => "memory_create",
            Syscall::MemoryMap => "memory_map",
            Syscall::MemoryUnmap => "memory_unmap",
            Syscall::ChannelCreate => "channel_create",
            Syscall::ChannelSend => "channel_send",
            Syscall::ChannelRecv => "channel_recv",
            Syscall::PortCreate => "port_create",
            Syscall::PortWait => "port_wait",
            Syscall::PortBind => "port_bind",
            Syscall::ProcessSpawn => "process_spawn",
            Syscall::ProcessExit => "process_exit",
            Syscall::ThreadCreate => "thread_create",
            Syscall::ThreadSleep => "thread_sleep",
            Syscall::ThreadYield => "thread_yield",
            Syscall::ClockRead => "clock_read",
        }
    }
}

/// Version dieser ABI. Wird von [`Syscall::Version`] zurueckgegeben.
pub const ABI_VERSION: u64 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_numbers_roundtrip() {
        for n in 0..=Syscall::MAX {
            let s = Syscall::from_raw(n).expect("Luecke in der Syscall-Tabelle");
            assert_eq!(s as u64, n);
            assert!(!s.name().is_empty());
        }
        assert!(Syscall::from_raw(Syscall::MAX + 1).is_none());
    }

    #[test]
    fn names_are_unique_and_in_snake_case() {
        // Die Namen landen in Diagnoseausgaben und spaeter in der libc-Bindung:
        // doppelte oder unsaubere Namen fallen hier auf, nicht erst dort.
        let mut seen = std::vec::Vec::new();
        for n in 0..=Syscall::MAX {
            let s = Syscall::from_raw(n).unwrap();
            let name = s.name();
            assert!(!seen.contains(&name), "Name {name} doppelt vergeben");
            assert!(
                name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "Name {name} ist nicht snake_case"
            );
            assert!(!name.starts_with('_') && !name.ends_with('_'), "Name {name} unsauber");
            seen.push(name);
        }
        assert_eq!(seen.len() as u64, Syscall::MAX + 1);
    }

    #[test]
    fn unknown_numbers_are_rejected_without_panicking() {
        // Ein Aufrufer setzt eine beliebige Zahl in das Nummernregister: der
        // Dispatch darf nur `None` liefern, nie danebengreifen.
        for n in [
            Syscall::MAX + 1,
            Syscall::MAX + 2,
            100,
            255,
            0x1_0000,
            u64::MAX / 2,
            u64::MAX - 1,
            u64::MAX,
        ] {
            assert!(Syscall::from_raw(n).is_none(), "Nummer {n} faelschlich akzeptiert");
        }
    }

    #[test]
    fn max_matches_the_table_size() {
        assert_eq!(Syscall::MAX, 22);
        assert_eq!(Syscall::from_raw(Syscall::MAX), Some(Syscall::ClockRead));
        assert_eq!(Syscall::from_raw(0), Some(Syscall::Version));
        // Luecken sind verboten: die Nummern 0..=MAX muessen alle belegt sein,
        // damit eine Sprungtabelle im Kernel ohne Loecher arbeiten kann.
        let filled = (0..=Syscall::MAX).filter(|n| Syscall::from_raw(*n).is_some()).count();
        assert_eq!(filled as u64, Syscall::MAX + 1);
    }

    #[test]
    fn the_native_abi_has_no_posix_shaped_calls() {
        // Kernaussage der Architektur: kein fork, kein ioctl, kein errno, keine
        // Signals in der NATIVEN ABI. Der Test haelt diese Zusage fest.
        for n in 0..=Syscall::MAX {
            let name = Syscall::from_raw(n).unwrap().name();
            for verboten in ["fork", "ioctl", "signal", "errno", "dup2", "select"] {
                assert!(!name.contains(verboten), "{name} riecht nach POSIX ({verboten})");
            }
        }
    }

    #[test]
    fn abi_version_is_reported_and_nonzero() {
        assert_eq!(ABI_VERSION, 1);
        assert!(ABI_VERSION > 0, "Version 0 waere nicht von 'unbekannt' zu trennen");
        assert_eq!(Syscall::Version as u64, 0);
    }

    #[test]
    fn syscalls_are_copy_and_comparable() {
        let a = Syscall::ChannelSend;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(Syscall::ChannelSend, Syscall::ChannelRecv);
        assert_eq!(Syscall::from_raw(12), Some(Syscall::ChannelSend));
    }
}
