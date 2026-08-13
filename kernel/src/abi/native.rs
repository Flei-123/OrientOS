//! Anbindung der nativen ABI an den Kernel — Prozesse, Objekte, Verteiler.
//!
//! **Schicht: abi.** Die ABI selbst (Typen, Nummern, Rechte, Handle-Tabelle)
//! liegt in der Crate [`karst_abi_native`] und ist host-getestet. Hier steht
//! die Kernelseite:
//!
//! * eine **Objekttafel** mit Verweiszaehler — Handles zeigen auf eine ZAHL,
//!   nie auf einen Zeiger,
//! * eine **Prozesstafel**, jeder Prozess mit EIGENER Handle-Tabelle und
//!   eigenem Wuerfelwert,
//! * `spawn` statt `fork`: ein neuer Prozess bekommt genau die Handles, die
//!   ihm der Erzeuger namentlich uebergibt — nichts wird geerbt,
//! * der Verteiler [`dispatch`], den der Systemaufrufpfad der Architektur
//!   aufruft (Nummer + sechs Argumente, Rueckgabe [`karst_abi_native::encode`]).
//!
//! Was es hier bewusst NICHT gibt: keinen globalen Namensraum, keine
//! Umgebungsrechte ("ambient authority"), kein `fork`, kein `errno`, keine
//! Sonderrollen fuer die Handles 0/1/2. Wer etwas darf, hat ein Handle dafuer.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use karst_abi_native::table::HandleTable;
use karst_abi_native::{
    encode, port::Packet, port::Signals, syscall::ABI_VERSION, Error, Handle, HandleEntry,
    ObjectKind, Result, Rights, Syscall, PACKET_BYTES,
};

use crate::kcore::arch_iface::TimerOps;
use crate::klog;

/// Groesste Nachricht, die ein Kanal traegt. Eine Grenze ist Pflicht: ohne sie
/// koennte ein Prozess den Kernelspeicher durch Senden erschoepfen.
const MAX_MESSAGE: usize = 4096;
/// Groesste Anzahl Handles in einer Nachricht.
const MAX_MESSAGE_HANDLES: usize = 4;
/// Groesste Nachrichtenzahl je Kanalende.
const MAX_QUEUE: usize = 16;
/// Groesste Zahl gebundener Objekte je Port.
const MAX_BINDINGS: usize = 16;
/// Groesste Zahl wartender Ereignisse je Port. Laeuft der Port ueber, gehen
/// keine Ereignisse verloren, sondern das aelteste bleibt stehen und das neue
/// wird abgewiesen — ein Ereignissturm darf keinen Speicher fressen.
const MAX_PACKETS: usize = 32;

// ---------------------------------------------------------------- Objekttafel

/// Inhalt eines Kernelobjekts. Ein Handle traegt nur die Nummer des Objekts;
/// diese Nummer wird hier — und nur hier — nachgeschlagen.
enum ObjectData {
    /// Platz frei (letztes Handle geschlossen).
    Free,
    /// Ausgabestrom auf die Systemkonsole.
    Console,
    /// Kanalende. `peer` ist die Objektnummer der Gegenseite.
    Channel { peer: u64, queue: VecDeque<Message> },
    /// Prozessobjekt (verweist auf einen Eintrag der Prozesstafel).
    Process { pid: u32 },
    /// Speicherobjekt fester Groesse.
    Memory { len: u64 },
    /// Wartepunkt fuer Ereignisse — der Ersatz fuer Signals. `binds` sind die
    /// vom Besitzer selbst eingetragenen Beobachtungen, `queue` die noch nicht
    /// abgeholten Pakete.
    Port { binds: Vec<Binding>, queue: VecDeque<Packet> },
}

/// Eine Beobachtung: "melde mir `mask` von Objekt `object` unter `key`".
/// Der Schluessel gehoert dem Wartenden; der Kernel gibt nie eine Objektnummer
/// nach draussen.
struct Binding {
    object: u64,
    key: u64,
    mask: Signals,
    kind: ObjectKind,
}

/// Eine Nachricht auf einem Kanal: Bytes plus explizit uebergebene Handles.
struct Message {
    bytes: Vec<u8>,
    handles: Vec<HandleEntry>,
}

struct Object {
    kind: ObjectKind,
    /// Wie viele Handles zeigen auf dieses Objekt? Beim Erreichen von 0 wird
    /// der Platz freigegeben — ein spaeter auftauchendes altes Handle findet
    /// dann nichts mehr, weil auch die Generation nicht mehr passt.
    refs: u32,
    data: ObjectData,
}

impl Object {
    const fn free() -> Self {
        Object { kind: ObjectKind::Memory, refs: 0, data: ObjectData::Free }
    }
}

// --------------------------------------------------------------- Prozesstafel

/// Ein Prozess aus Sicht der nativen ABI.
pub struct Process {
    /// Nummer des Prozesses in der Prozesstafel.
    pub pid: u32,
    /// Name fuer Diagnoseausgaben.
    pub name: &'static str,
    /// Laeuft der Prozess unprivilegiert? Nur dann gelten Zeiger aus Argumenten
    /// als unprivilegierte Adressen.
    pub user: bool,
    handles: HandleTable,
    exit_code: Option<i64>,
}

impl Process {
    /// Anzahl der Handles, die dieser Prozess besitzt.
    pub fn handle_count(&self) -> usize {
        self.handles.len()
    }
}

/// Der gesamte veraenderliche Zustand der nativen ABI.
struct Native {
    objects: Vec<Object>,
    procs: Vec<Process>,
    current: u32,
    started: bool,
    syscalls: u64,
    denied: u64,
    user_denied: u64,
    spawns: u64,
    /// Zustand des Wuerfels fuer die prozesseigenen Handle-Nonces.
    seed: u64,
}

impl Native {
    const fn new() -> Self {
        Native {
            objects: Vec::new(),
            procs: Vec::new(),
            current: 0,
            started: false,
            syscalls: 0,
            denied: 0,
            user_denied: 0,
            spawns: 0,
            seed: 0x2545_f491_4f6c_dd1d,
        }
    }

    /// Wuerfelt einen Wert fuer die Handle-Nonce eines Prozesses.
    /// Bewusst ein eigener 4-Zeiler statt einer Zufallscrate.
    fn next_nonce(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed = self.seed.wrapping_add(<crate::arch::Timer as TimerOps>::ticks());
        (self.seed >> 32) as u32 | 1
    }

    fn ensure_started(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        // Der Wurzelprozess ist der Kernel selbst. Auch er hat keine
        // Umgebungsrechte: seine Konsole ist ein Handle wie jedes andere.
        let pid = self.new_process("systemkern", false);
        let obj = self.new_object(ObjectKind::Stream, ObjectData::Console);
        let _ = self.grant(pid, obj, Rights::ALL);
        self.current = pid;
    }

    fn new_process(&mut self, name: &'static str, user: bool) -> u32 {
        let nonce = self.next_nonce();
        let pid = self.procs.len() as u32;
        self.procs.push(Process {
            pid,
            name,
            user,
            handles: HandleTable::with_nonce(karst_abi_native::table::DEFAULT_LIMIT, nonce),
            exit_code: None,
        });
        pid
    }

    fn new_object(&mut self, kind: ObjectKind, data: ObjectData) -> u64 {
        if let Some(i) = self.objects.iter().position(|o| matches!(o.data, ObjectData::Free)) {
            self.objects[i] = Object { kind, refs: 0, data };
            return i as u64;
        }
        self.objects.push(Object { kind, refs: 0, data });
        (self.objects.len() - 1) as u64
    }

    /// Traegt ein bestehendes Objekt in die Tabelle eines Prozesses ein.
    /// Die Objektart kommt aus der Objekttafel, nicht vom Aufrufer — ein
    /// Handle kann damit nie eine falsche Art vorgeben.
    fn grant(&mut self, pid: u32, obj: u64, rights: Rights) -> Result<Handle> {
        let kind = self.objects.get(obj as usize).ok_or(Error::NotFound)?.kind;
        let p = self.procs.get_mut(pid as usize).ok_or(Error::NotFound)?;
        let h = p.handles.insert(kind, rights, obj)?;
        if let Some(o) = self.objects.get_mut(obj as usize) {
            o.refs += 1;
        }
        Ok(h)
    }

    fn proc_mut(&mut self, pid: u32) -> Result<&mut Process> {
        self.procs.get_mut(pid as usize).ok_or(Error::NotFound)
    }

    /// Gibt einen Verweis frei; beim letzten Verweis verschwindet das Objekt.
    fn release(&mut self, obj: u64) {
        let Some(o) = self.objects.get_mut(obj as usize) else { return };
        if o.refs > 0 {
            o.refs -= 1;
        }
        if o.refs == 0 {
            // Kanalende weg: die Gegenseite erfaehrt das ueber ihren Port und
            // spaetestens beim naechsten Senden.
            let peer = match o.data {
                ObjectData::Channel { peer, .. } => Some(peer),
                _ => None,
            };
            *o = Object::free();
            // Objektnummern werden wiederverwendet. Alle Beobachtungen auf
            // diese Nummer muessen deshalb verschwinden — sonst wuerde ein
            // altes Binding spaeter Ereignisse eines FREMDEN Objekts melden.
            self.unbind_all(obj);
            if let Some(peer) = peer {
                self.notify_ports(peer, Signals::PEER_CLOSED);
            }
        }
    }

    /// Entfernt alle Beobachtungen auf eine Objektnummer aus allen Ports.
    fn unbind_all(&mut self, obj: u64) {
        for o in self.objects.iter_mut() {
            if let ObjectData::Port { binds, .. } = &mut o.data {
                binds.retain(|b| b.object != obj);
            }
        }
    }

    /// Meldet ein Ereignis eines Objekts an alle Ports, die es beobachten.
    /// Ein Port bekommt nur, was sein Besitzer selbst gebunden hat — es gibt
    /// keinen Weg, jemandem ungefragt ein Ereignis zuzustellen.
    fn notify_ports(&mut self, obj: u64, sig: Signals) {
        for o in self.objects.iter_mut() {
            let ObjectData::Port { binds, queue } = &mut o.data else { continue };
            for b in binds.iter() {
                if b.object != obj || !b.mask.contains(sig) {
                    continue;
                }
                if queue.len() >= MAX_PACKETS {
                    break;
                }
                queue.push_back(Packet::new(b.key, sig, b.kind));
            }
        }
    }

    fn close(&mut self, pid: u32, h: Handle) -> Result<()> {
        let e = self.proc_mut(pid)?.handles.close(h)?;
        self.release(e.object);
        Ok(())
    }
}

static NATIVE: spin::Mutex<Native> = spin::Mutex::new(Native::new());

/// Sperrt den ABI-Zustand und stellt sicher, dass der Wurzelprozess existiert.
fn lock() -> spin::MutexGuard<'static, Native> {
    let mut g = NATIVE.lock();
    g.ensure_started();
    g
}

// ------------------------------------------------------------ oeffentliche API

/// Erzeugt einen Prozess im spawn-Stil: **keine Kopie eines Adressraums, kein
/// Erben**. Der Erzeuger listet die Handles auf, die er abgeben will, jeweils
/// mit einer Rechtemaske, die die Rechte nur verkleinern kann.
///
/// Fehlt einem der genannten Handles das Recht [`Rights::TRANSFER`], scheitert
/// der ganze Aufruf — und der neue Prozess bekommt gar nichts.
pub fn spawn(parent: u32, name: &'static str, grants: &[(Handle, Rights)]) -> Result<u32> {
    spawn_with(parent, name, true, grants)
}

/// Wie [`spawn`], aber mit der Wahl, ob der neue Prozess unprivilegiert laeuft.
/// Kerneldienste (`user = false`) duerfen Puffer im Kernelspeicher benutzen;
/// unprivilegierte Prozesse niemals.
pub fn spawn_with(
    parent: u32,
    name: &'static str,
    user: bool,
    grants: &[(Handle, Rights)],
) -> Result<u32> {
    let mut n = lock();
    if grants.len() > karst_abi_native::table::DEFAULT_LIMIT {
        return Err(Error::InvalidArgs);
    }
    // Erst pruefen, dann handeln: ein Fehler in der Mitte darf keinen halb
    // ausgestatteten Prozess hinterlassen.
    {
        let p = n.procs.get(parent as usize).ok_or(Error::NotFound)?;
        for (h, _) in grants {
            p.handles.get_with(*h, Rights::TRANSFER)?;
        }
    }
    let pid = n.new_process(name, user);
    for (h, mask) in grants {
        let entry = n.proc_mut(parent)?.handles.take_for_transfer(*h, *mask)?;
        let neu = n.proc_mut(pid)?.handles.adopt(entry)?;
        debug_assert!(neu.is_valid());
    }
    // Der Erzeuger bekommt ein Handle auf den neuen Prozess — auch das ist ein
    // gewoehnliches Objekt mit Rechten, keine Sonderbeziehung.
    let obj = n.new_object(ObjectKind::Process, ObjectData::Process { pid });
    let _ = n.grant(parent, obj, Rights::INSPECT | Rights::MANAGE | Rights::WAIT);
    n.spawns += 1;
    Ok(pid)
}

/// Prozess des laufenden Aufrufs.
pub fn current() -> u32 {
    lock().current
}

/// Setzt den Prozess, dem die naechsten Systemaufrufe zugerechnet werden.
/// Liefert den vorherigen Wert.
pub fn set_current(pid: u32) -> u32 {
    let mut n = lock();
    let alt = n.current;
    if (pid as usize) < n.procs.len() {
        n.current = pid;
    }
    alt
}

/// Beendet einen Prozess: alle Handles fallen weg, die Objekte verlieren ihre
/// Verweise. Danach ist mit den alten Handle-Werten nichts mehr zu holen.
pub fn exit_process(pid: u32, code: i64) -> Result<()> {
    let handles: Vec<Handle> = {
        let mut n = lock();
        let p = n.procs.get_mut(pid as usize).ok_or(Error::NotFound)?;
        p.exit_code = Some(code);
        p.handles.iter().map(|e| e.handle).collect()
    };
    let mut n = lock();
    for h in handles {
        if let Ok(e) = n.proc_mut(pid)?.handles.close(h) {
            n.release(e.object);
        }
    }
    if n.current == pid {
        n.current = 0;
    }
    // Wer ein Prozessobjekt an einem Port beobachtet, erfaehrt das Ende.
    let prozessobjekte: Vec<u64> = n
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o.data, ObjectData::Process { pid: p } if p == pid))
        .map(|(i, _)| i as u64)
        .collect();
    for obj in prozessobjekte {
        n.notify_ports(obj, Signals::PROCESS_EXIT);
    }
    Ok(())
}

/// Anzahl Handles eines Prozesses (fuer Diagnose und Tests).
pub fn handle_count(pid: u32) -> usize {
    lock().procs.get(pid as usize).map(|p| p.handles.len()).unwrap_or(0)
}

/// Erzeugt ein Konsolenobjekt und gibt dem Prozess ein Handle darauf.
pub fn grant_console(pid: u32, rights: Rights) -> Result<Handle> {
    let mut n = lock();
    let obj = n.new_object(ObjectKind::Stream, ObjectData::Console);
    n.grant(pid, obj, rights)
}

/// Erzeugt ein verbundenes Kanalpaar und gibt beide Enden dem Prozess.
pub fn channel_create(pid: u32) -> Result<(Handle, Handle)> {
    let mut n = lock();
    let a = n.new_object(ObjectKind::Channel, ObjectData::Channel { peer: 0, queue: VecDeque::new() });
    let b = n.new_object(ObjectKind::Channel, ObjectData::Channel { peer: a, queue: VecDeque::new() });
    if let Some(o) = n.objects.get_mut(a as usize) {
        if let ObjectData::Channel { peer, .. } = &mut o.data {
            *peer = b;
        }
    }
    let rights = Rights::READ
        | Rights::WRITE
        | Rights::TRANSFER
        | Rights::DUPLICATE
        | Rights::INSPECT
        | Rights::WAIT;
    let ha = n.grant(pid, a, rights)?;
    let hb = n.grant(pid, b, rights)?;
    Ok((ha, hb))
}

// ------------------------------------------------------------------ Verteiler

/// Systemaufruf aus der unprivilegierten Ebene.
///
/// Zeiger in den Argumenten werden als **unprivilegierte** Adressen behandelt
/// und geprueft; eine Kerneladresse wird nie gelesen.
pub fn dispatch(nr: u64, args: [u64; 6]) -> i64 {
    encode(call(nr, args, true))
}

/// Systemaufruf aus dem Kernel selbst (Selbsttests, Dienste). Hier duerfen
/// Zeiger auf Kernelspeicher zeigen — der Aufrufer ist der Kernel.
pub fn dispatch_kernel(nr: u64, args: [u64; 6]) -> i64 {
    encode(call(nr, args, false))
}

fn call(nr: u64, args: [u64; 6], from_user: bool) -> Result<u64> {
    let Some(sc) = Syscall::from_raw(nr) else {
        lock().denied += 1;
        return Err(Error::InvalidArgs);
    };
    let (pid, ist_user) = {
        let mut n = lock();
        n.syscalls += 1;
        let cur = n.current;
        let user = n.procs.get(cur as usize).map(|p| p.user).unwrap_or(false);
        (cur, user)
    };
    // Ein unprivilegierter Prozess bekommt IMMER die strenge Zeigerpruefung,
    // auch wenn der Aufruf ueber den Kernelpfad hereinkommt.
    let r = perform(pid, sc, args, from_user || ist_user);
    if let Err(e) = r {
        let mut n = lock();
        n.denied += 1;
        if ist_user {
            n.user_denied += 1;
            let name = n.procs.get(pid as usize).map(|p| p.name).unwrap_or("?");
            drop(n);
            // Sichtbarer Beleg: ein unprivilegierter Aufrufer wurde an der
            // Capability-Pruefung abgewiesen — mit Aufruf, Prozess und Grund.
            klog!(
                "abi",
                "Aufruf {} aus unprivilegiertem Prozess {} \"{}\" abgewiesen: {}",
                sc.name(),
                pid,
                name,
                e.name()
            );
        }
    }
    r
}

fn perform(pid: u32, sc: Syscall, args: [u64; 6], from_user: bool) -> Result<u64> {
    match sc {
        Syscall::Version => Ok(ABI_VERSION),
        Syscall::ThreadYield => {
            crate::kcore::sched::yield_now();
            Ok(0)
        }
        Syscall::ClockRead => Ok(now_ns()),
        Syscall::HandleClose => {
            let mut n = lock();
            n.close(pid, Handle(args[0]))?;
            Ok(0)
        }
        Syscall::HandleDuplicate => {
            let mut n = lock();
            let h = Handle(args[0]);
            let obj = n.procs.get(pid as usize).ok_or(Error::NotFound)?.handles.get(h)?.object;
            let neu = n.proc_mut(pid)?.handles.duplicate(h, Rights(args[1] as u32))?;
            if let Some(o) = n.objects.get_mut(obj as usize) {
                o.refs += 1;
            }
            Ok(neu.0)
        }
        Syscall::HandleInspect => {
            let n = lock();
            let e = n
                .procs
                .get(pid as usize)
                .ok_or(Error::NotFound)?
                .handles
                .get_with(Handle(args[0]), Rights::INSPECT)?;
            // Verdichtete Auskunft im Rueckgabekanal: Typ und Rechte.
            // Kein Zeiger nach draussen, keine Kerneladresse.
            Ok(((e.kind as u64) << 32) | e.rights.bits() as u64)
        }
        Syscall::HandleWrite => {
            let (obj, _) = {
                let n = lock();
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::WRITE)?;
                (e.object, e.kind)
            };
            let bytes = read_buffer(args[1], args[2], from_user)?;
            let mut n = lock();
            let o = n.objects.get_mut(obj as usize).ok_or(Error::NotFound)?;
            match &mut o.data {
                ObjectData::Console => {
                    let len = bytes.len();
                    drop(n);
                    if len > 0 {
                        print_from_user(&bytes);
                    }
                    Ok(len as u64)
                }
                ObjectData::Channel { peer, .. } => {
                    let peer = *peer;
                    let len = bytes.len();
                    push_message(&mut n, peer, Message { bytes, handles: Vec::new() })?;
                    Ok(len as u64)
                }
                _ => Err(Error::WrongType),
            }
        }
        Syscall::HandleRead => {
            let obj = {
                let n = lock();
                n.procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::READ)?
                    .object
            };
            let mut n = lock();
            let o = n.objects.get_mut(obj as usize).ok_or(Error::NotFound)?;
            match &mut o.data {
                // Die Konsole ist in diesem Stand nur Ausgabe — ehrlich gemeldet.
                ObjectData::Console => Err(Error::WouldBlock),
                ObjectData::Channel { queue, .. } => {
                    let msg = queue.pop_front().ok_or(Error::WouldBlock)?;
                    let len = msg.bytes.len().min(args[2] as usize);
                    drop(n);
                    // Mitgeschickte Handles brauchen Platz im Ausgabefeld des
                    // Empfaengers. Passt es nicht, bleibt die Nachricht liegen —
                    // ein Handle darf nie ankommen, ohne dass der Empfaenger
                    // seinen Wert erfaehrt.
                    let platz = if msg.handles.is_empty() {
                        Ok(())
                    } else if msg.handles.len() > args[4] as usize {
                        Err(Error::InvalidArgs)
                    } else {
                        check_range(args[3], (msg.handles.len() * 8) as u64, from_user)
                    };
                    if let Err(e) = platz {
                        let mut n = lock();
                        if let Some(o) = n.objects.get_mut(obj as usize) {
                            if let ObjectData::Channel { queue, .. } = &mut o.data {
                                queue.push_front(msg);
                            }
                        }
                        return Err(e);
                    }
                    if let Err(e) = write_buffer(args[1], &msg.bytes[..len], from_user) {
                        // Zielpuffer untauglich: die Nachricht bleibt erhalten,
                        // statt beim Fehlversuch verloren zu gehen.
                        let mut n = lock();
                        if let Some(o) = n.objects.get_mut(obj as usize) {
                            if let ObjectData::Channel { queue, .. } = &mut o.data {
                                queue.push_front(msg);
                            }
                        }
                        return Err(e);
                    }
                    // Mitgeschickte Handles gehen in die Tabelle des
                    // Empfaengers — mit NEUEN Werten. Die Werte des Senders
                    // gelten dort nicht und sind bei ihm bereits ungueltig.
                    let mut neue = Vec::new();
                    {
                        let mut n = lock();
                        for e in msg.handles {
                            match n.proc_mut(pid)?.handles.adopt(e) {
                                Ok(h) => neue.push(h),
                                Err(_) => n.release(e.object),
                            }
                        }
                    }
                    for (i, h) in neue.iter().enumerate() {
                        write_word(args[3] + (i as u64) * 8, h.0, from_user)?;
                    }
                    Ok(len as u64)
                }
                _ => Err(Error::WrongType),
            }
        }
        Syscall::ChannelCreate => {
            let (a, b) = channel_create(pid)?;
            write_word(args[0], a.0, from_user)?;
            write_word(args[1], b.0, from_user)?;
            Ok(0)
        }
        Syscall::ChannelSend => {
            let obj = {
                let n = lock();
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::WRITE)?;
                if e.kind != ObjectKind::Channel {
                    return Err(Error::WrongType);
                }
                e.object
            };
            let bytes = read_buffer(args[1], args[2], from_user)?;
            let count = args[4] as usize;
            if count > MAX_MESSAGE_HANDLES {
                return Err(Error::InvalidArgs);
            }
            // Erst ALLE genannten Handles einlesen und pruefen, dann erst
            // wegnehmen: ein Fehler in der Mitte darf keinen Handle
            // verschwinden lassen, ohne dass die Nachricht ankommt.
            let mut genannt = Vec::new();
            for i in 0..count {
                let raw = read_word(args[3] + (i as u64) * 8, from_user)?;
                let n = lock();
                n.procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(raw), Rights::TRANSFER)?;
                if genannt.contains(&Handle(raw)) {
                    // Zweimal dasselbe Handle in einer Nachricht: der zweite
                    // Griff wuerde ins Leere gehen. Lieber gleich abweisen.
                    return Err(Error::InvalidArgs);
                }
                genannt.push(Handle(raw));
            }
            let mut mitgegeben = Vec::new();
            for h in genannt {
                let mut n = lock();
                let e = n.proc_mut(pid)?.handles.take_for_transfer(h, Rights::ALL)?;
                mitgegeben.push(e);
            }
            let mut n = lock();
            let peer = match n.objects.get(obj as usize).map(|o| &o.data) {
                Some(ObjectData::Channel { peer, .. }) => *peer,
                _ => return Err(Error::WrongType),
            };
            let len = bytes.len();
            push_message(&mut n, peer, Message { bytes, handles: mitgegeben })?;
            Ok(len as u64)
        }
        Syscall::ChannelRecv => perform(pid, Syscall::HandleRead, args, from_user),
        Syscall::MemoryCreate => {
            let len = args[0];
            if len == 0 || len > 1 << 30 {
                return Err(Error::InvalidArgs);
            }
            let mut n = lock();
            let obj = n.new_object(ObjectKind::Memory, ObjectData::Memory { len });
            let h = n.grant(
                pid,
                obj,
                Rights::READ | Rights::WRITE | Rights::MAP | Rights::TRANSFER | Rights::INSPECT,
            )?;
            Ok(h.0)
        }
        Syscall::ProcessExit => {
            // `(0, code)` beendet den eigenen Prozess; mit einem Prozesshandle
            // und dem Recht MANAGE laesst sich ein anderer beenden.
            let ziel = if args[0] == 0 {
                pid
            } else {
                let n = lock();
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::MANAGE)?;
                match n.objects.get(e.object as usize).map(|o| &o.data) {
                    Some(ObjectData::Process { pid }) => *pid,
                    _ => return Err(Error::WrongType),
                }
            };
            exit_process(ziel, args[1] as i64)?;
            Ok(0)
        }
        Syscall::PortCreate => {
            let mut n = lock();
            let obj = n.new_object(
                ObjectKind::Port,
                ObjectData::Port { binds: Vec::new(), queue: VecDeque::new() },
            );
            let h = n.grant(
                pid,
                obj,
                Rights::READ
                    | Rights::WAIT
                    | Rights::MANAGE
                    | Rights::TRANSFER
                    | Rights::DUPLICATE
                    | Rights::INSPECT,
            )?;
            Ok(h.0)
        }
        Syscall::PortBind => {
            // `(port, objekt, key, signale)`. Zwei Rechte muessen zusammen
            // kommen: MANAGE am Port (ich darf ihn einrichten) und WAIT am
            // Objekt (ich darf dessen Ereignisse ueberhaupt sehen).
            let maske = Signals(args[3] as u32).restrict(Signals::ALL);
            if maske.is_empty() {
                return Err(Error::InvalidArgs);
            }
            let mut n = lock();
            let port = {
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::MANAGE)?;
                if e.kind != ObjectKind::Port {
                    return Err(Error::WrongType);
                }
                e.object
            };
            let (ziel, art) = {
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[1]), Rights::WAIT)?;
                (e.object, e.kind)
            };
            let bereit = matches!(
                n.objects.get(ziel as usize).map(|o| &o.data),
                Some(ObjectData::Channel { queue, .. }) if !queue.is_empty()
            );
            let o = n.objects.get_mut(port as usize).ok_or(Error::NotFound)?;
            let ObjectData::Port { binds, queue } = &mut o.data else {
                return Err(Error::WrongType);
            };
            if binds.len() >= MAX_BINDINGS {
                return Err(Error::Exhausted);
            }
            binds.push(Binding { object: ziel, key: args[2], mask: maske, kind: art });
            // Was schon anliegt, geht beim Binden nicht verloren.
            if bereit && maske.contains(Signals::READABLE) && queue.len() < MAX_PACKETS {
                queue.push_back(Packet::new(args[2], Signals::READABLE, art));
            }
            Ok(0)
        }
        Syscall::PortWait => {
            // `(port, paketpuffer, frist_ns)`. Wartet, bis ein Ereignis
            // vorliegt oder die Frist abgelaufen ist. `frist_ns == 0` fragt
            // nur nach, ohne zu warten.
            let port = {
                let n = lock();
                let e = n
                    .procs
                    .get(pid as usize)
                    .ok_or(Error::NotFound)?
                    .handles
                    .get_with(Handle(args[0]), Rights::WAIT)?;
                if e.kind != ObjectKind::Port {
                    return Err(Error::WrongType);
                }
                e.object
            };
            check_range(args[1], PACKET_BYTES as u64, from_user)?;
            loop {
                let paket = {
                    let mut n = lock();
                    let o = n.objects.get_mut(port as usize).ok_or(Error::Closed)?;
                    match &mut o.data {
                        ObjectData::Port { queue, .. } => queue.pop_front(),
                        _ => return Err(Error::Closed),
                    }
                };
                if let Some(p) = paket {
                    write_buffer(args[1], &p.to_bytes(), from_user)?;
                    return Ok(1);
                }
                if now_ns() >= args[2] {
                    return Err(Error::WouldBlock);
                }
                // Warten heisst hier: die CPU abgeben. Der Zeitgeber bringt
                // uns zurueck, die Frist wird oben geprueft.
                crate::kcore::sched::yield_now();
            }
        }
        Syscall::ThreadSleep => {
            // `(frist_ns)` — absolute Frist. Eine Frist in der Vergangenheit
            // ist kein Fehler, sondern schlicht sofort vorbei.
            let jetzt = now_ns();
            if args[0] <= jetzt {
                return Ok(0);
            }
            let hz = <crate::arch::Timer as TimerOps>::hz() as u64;
            let ticks = ((args[0] - jetzt).saturating_mul(hz) / 1_000_000_000).max(1);
            crate::kcore::sched::sleep_ticks(ticks);
            Ok(0)
        }
        // Diese Aufrufe brauchen Teile, die dieser Stand noch nicht hat
        // (Adressraumobjekte, Namensraumdienst, Threads aus Ring 3).
        // Sie melden das sauber — siehe ROADMAP.md.
        Syscall::MemoryMap
        | Syscall::MemoryUnmap
        | Syscall::NamespaceOpen
        | Syscall::NamespaceCreate
        | Syscall::ProcessSpawn
        | Syscall::ThreadCreate => Err(Error::NotSupported),
    }
}

/// Zeit seit dem Start in Nanosekunden — die einzige Zeitquelle der ABI.
fn now_ns() -> u64 {
    <crate::arch::Timer as TimerOps>::ticks()
        .saturating_mul(1_000_000_000 / <crate::arch::Timer as TimerOps>::hz() as u64)
}

fn push_message(n: &mut Native, obj: u64, msg: Message) -> Result<()> {
    let o = n.objects.get_mut(obj as usize).ok_or(Error::Closed)?;
    match &mut o.data {
        ObjectData::Channel { queue, .. } => {
            if queue.len() >= MAX_QUEUE {
                return Err(Error::WouldBlock);
            }
            queue.push_back(msg);
            // Wer dieses Kanalende an einem Port beobachtet, erfaehrt jetzt,
            // dass etwas zu lesen ist.
            n.notify_ports(obj, Signals::READABLE);
            Ok(())
        }
        _ => Err(Error::Closed),
    }
}

// ------------------------------------------------------- Puffer aus dem Aufruf

/// Prueft einen Pufferbereich und liefert eine Kopie.
///
/// Aus der unprivilegierten Ebene sind ausschliesslich unprivilegierte
/// Adressen erlaubt — ein Zeiger auf Kernelspeicher wird abgewiesen, bevor er
/// gelesen wird (das ist die zweite Verteidigungslinie neben der Seitentabelle).
fn read_buffer(ptr: u64, len: u64, from_user: bool) -> Result<Vec<u8>> {
    let len = len as usize;
    if len > MAX_MESSAGE {
        return Err(Error::InvalidArgs);
    }
    if len == 0 {
        return Ok(Vec::new());
    }
    check_range(ptr, len as u64, from_user)?;
    let mut v = Vec::new();
    v.try_reserve(len).map_err(|_| Error::Exhausted)?;
    // SAFETY: Der Bereich wurde geprueft (Laenge begrenzt, im erlaubten
    // Adressbereich) und wird nur gelesen.
    let src = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    v.extend_from_slice(src);
    Ok(v)
}

fn write_buffer(ptr: u64, data: &[u8], from_user: bool) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    check_range(ptr, data.len() as u64, from_user)?;
    // SAFETY: gepruefter Bereich, Laenge passt zur Quelle.
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
    }
    Ok(())
}

fn read_word(ptr: u64, from_user: bool) -> Result<u64> {
    check_range(ptr, 8, from_user)?;
    if ptr % 8 != 0 {
        return Err(Error::InvalidArgs);
    }
    // SAFETY: geprueft und ausgerichtet.
    Ok(unsafe { core::ptr::read_volatile(ptr as *const u64) })
}

fn write_word(ptr: u64, value: u64, from_user: bool) -> Result<()> {
    check_range(ptr, 8, from_user)?;
    if ptr % 8 != 0 {
        return Err(Error::InvalidArgs);
    }
    // SAFETY: geprueft und ausgerichtet.
    unsafe { core::ptr::write_volatile(ptr as *mut u64, value) };
    Ok(())
}

fn check_range(ptr: u64, len: u64, from_user: bool) -> Result<()> {
    if ptr == 0 || len == 0 {
        return Err(Error::InvalidArgs);
    }
    let end = ptr.checked_add(len - 1).ok_or(Error::InvalidArgs)?;
    if from_user && !(crate::kcore::user::is_user_addr(ptr) && crate::kcore::user::is_user_addr(end))
    {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

/// Gibt Bytes eines Aufrufers auf der Systemkonsole aus. Nicht darstellbare
/// Zeichen werden ersetzt: ein Aufrufer soll die Konsole nicht steuern koennen.
fn print_from_user(bytes: &[u8]) {
    let mut buf = [0u8; 128];
    let mut i = 0;
    for &b in bytes.iter() {
        buf[i] = if (0x20..0x7f).contains(&b) { b } else { b'.' };
        i += 1;
        if i == buf.len() {
            break;
        }
    }
    let text = core::str::from_utf8(&buf[..i]).unwrap_or("<nicht darstellbar>");
    klog!("abi", "Ausgabe eines Aufrufers ueber Handle: \"{}\"", text);
}

// ------------------------------------------------------------- Selbsttests

fn ergebnis(b: bool) -> &'static str {
    if b {
        "ok"
    } else {
        "FEHLER"
    }
}

/// Negativtest der Handle-Tabelle: die Wege, ein Handle zu faelschen oder sich
/// ein Recht zu nehmen, das man nicht hat, muessen ALLE mit einem definierten
/// Fehler enden — und keiner davon darf Zugriff gewaehren.
///
/// Geprueft werden nacheinander
/// 1. ein Index, den es nie gab,
/// 2. eine veraltete Generation auf einem wiederverwendeten Platz,
/// 3. ein gueltiges Handle ohne das benoetigte Recht,
/// 4. ein Handle aus einem FREMDEN Prozess,
/// 5. ein frisch erzeugter Prozess ohne uebergebene Handles (keine
///    Umgebungsrechte),
/// 6. Rechteausweitung beim Duplizieren,
/// 7. Duplizieren ohne das Recht dazu,
/// 8. Uebergeben ohne das Recht dazu,
/// 9. eine unbekannte Aufrufnummer,
/// 10. Erraten der Generation (4096 Versuche auf einem bekannten Index),
/// 11. Benutzung nach dem Schliessen,
/// 12. Benutzung eines Handles eines beendeten Prozesses.
///
/// Liefert `(bestanden, gesamt)` fuer die Selbsttestbilanz.
pub fn capability_selftest() -> (usize, usize) {
    let root = current();
    let konsole = grant_console(root, Rights::ALL).unwrap_or(Handle(0));

    // 1. Index frei erfunden.
    let case1 = matches!(lookup(root, Handle::new(4242, 1), Rights::READ), Err(Error::BadHandle));

    // 2. Platz wiederverwenden: das alte Handle bleibt ungueltig.
    let alt = grant_console(root, Rights::READ).unwrap_or(Handle(0));
    let _ = dispatch_kernel(Syscall::HandleClose as u64, [alt.0, 0, 0, 0, 0, 0]);
    let frisch = grant_console(root, Rights::READ).unwrap_or(Handle(0));
    let case2 = frisch.slot() == alt.slot()
        && frisch.generation() != alt.generation()
        && matches!(lookup(root, alt, Rights::READ), Err(Error::BadHandle));

    // 3. Recht fehlt: Schreiben auf ein reines Lesehandle.
    let nur_lesen = grant_console(root, Rights::READ).unwrap_or(Handle(0));
    let case3 = matches!(lookup(root, nur_lesen, Rights::WRITE), Err(Error::RightsDenied));

    // 4. Fremdes Handle: ein zweiter Prozess bekommt EIN Handle explizit
    //    uebergeben und versucht danach, ein Handle des Erzeugers zu benutzen.
    let uebergabe = grant_console(root, Rights::WRITE | Rights::TRANSFER).unwrap_or(Handle(0));
    let kind = spawn(root, "gast", &[(uebergabe, Rights::WRITE)]);
    let case4 = match kind {
        Ok(gast) => {
            let vorher = set_current(gast);
            let fremd = dispatch_kernel(Syscall::HandleWrite as u64, [konsole.0, 0, 0, 0, 0, 0]);
            let eigenes = handle_count(gast) == 1;
            set_current(vorher);
            fremd == Error::BadHandle.as_raw() && eigenes
        }
        Err(_) => false,
    };

    // 5. Keine Umgebungsrechte: ein Prozess ohne Uebergabe hat NICHTS.
    let case5 = match spawn(root, "nackt", &[]) {
        Ok(leer) => {
            let vorher = set_current(leer);
            // Alle plausiblen kleinen Handle-Werte durchprobieren.
            let mut treffer = 0;
            for slot in 0..8u32 {
                for g in 0..8u32 {
                    let h = Handle::new(slot, g);
                    if lookup(leer, h, Rights::NONE).is_ok() {
                        treffer += 1;
                    }
                }
            }
            let schreiben =
                dispatch_kernel(Syscall::HandleWrite as u64, [Handle::new(0, 1).0, 0, 0, 0, 0, 0]);
            set_current(vorher);
            treffer == 0 && handle_count(leer) == 0 && schreiben < 0
        }
        Err(_) => false,
    };

    // 6./7. Duplizieren: Maske kann nur verkleinern, und ohne DUPLICATE
    //       geht gar nichts.
    let voll = grant_console(root, Rights::WRITE | Rights::DUPLICATE).unwrap_or(Handle(0));
    let dup = dispatch_kernel(Syscall::HandleDuplicate as u64, [voll.0, Rights::ALL.bits() as u64, 0, 0, 0, 0]);
    let case6 = dup >= 0
        && lookup(root, Handle(dup as u64), Rights::READ).is_err()
        && lookup(root, Handle(dup as u64), Rights::WRITE).is_ok();
    let ohne_dup = grant_console(root, Rights::WRITE).unwrap_or(Handle(0));
    let case7 = dispatch_kernel(
        Syscall::HandleDuplicate as u64,
        [ohne_dup.0, Rights::ALL.bits() as u64, 0, 0, 0, 0],
    ) == Error::RightsDenied.as_raw();

    // 8. Uebergeben ohne TRANSFER-Recht.
    let case8 = matches!(spawn(root, "dieb", &[(ohne_dup, Rights::ALL)]), Err(Error::RightsDenied));

    // 9. Unbekannte Aufrufnummer.
    let case9 = dispatch_kernel(Syscall::MAX + 1, [0; 6]) == Error::InvalidArgs.as_raw()
        && dispatch_kernel(u64::MAX, [0; 6]) == Error::InvalidArgs.as_raw();

    // 10. Generation raten: 4096 Versuche auf einem bekannten Index.
    let mut geraten = 0usize;
    for g in 0..4096u32 {
        if g == konsole.generation() {
            continue;
        }
        if lookup(root, Handle::new(konsole.slot(), g), Rights::NONE).is_ok() {
            geraten += 1;
        }
    }
    let case10 = geraten == 0;

    // 11. Benutzung nach dem Schliessen.
    let zu = grant_console(root, Rights::WRITE).unwrap_or(Handle(0));
    let _ = dispatch_kernel(Syscall::HandleClose as u64, [zu.0, 0, 0, 0, 0, 0]);
    let case11 = dispatch_kernel(Syscall::HandleWrite as u64, [zu.0, 0, 0, 0, 0, 0])
        == Error::BadHandle.as_raw()
        && dispatch_kernel(Syscall::HandleClose as u64, [zu.0, 0, 0, 0, 0, 0])
            == Error::BadHandle.as_raw();

    // 12. Handles eines beendeten Prozesses sind weg.
    let case12 = match spawn(root, "kurzlebig", &[]) {
        Ok(p) => {
            let h = grant_console(p, Rights::WRITE).unwrap_or(Handle(0));
            let vorher = set_current(p);
            let vor_ende = dispatch_kernel(Syscall::HandleWrite as u64, [h.0, 0, 0, 0, 0, 0]);
            let _ = exit_process(p, 0);
            set_current(p);
            let nach_ende = dispatch_kernel(Syscall::HandleWrite as u64, [h.0, 0, 0, 0, 0, 0]);
            set_current(vorher);
            // Vor dem Ende ist der Aufruf gueltig (leerer Puffer, 0 Bytes),
            // danach ist das Handle selbst ungueltig.
            vor_ende == 0
                && nach_ende == Error::BadHandle.as_raw()
                && handle_count(p) == 0
        }
        Err(_) => false,
    };

    let faelle = [
        ("ungueltiger Index", case1),
        ("veraltete Generation", case2),
        ("fehlendes Recht", case3),
        ("fremdes Handle", case4),
        ("ohne Uebergabe kein Zugriff", case5),
        ("keine Rechteausweitung", case6),
        ("Duplizieren ohne Recht", case7),
        ("Uebergabe ohne Recht", case8),
        ("unbekannte Aufrufnummer", case9),
        ("Generation geraten", case10),
        ("Benutzung nach Schliessen", case11),
        ("Handle nach Prozessende", case12),
    ];
    let ok = faelle.iter().filter(|(_, b)| *b).count();

    // Die drei Pflichtfaelle in genau der Form, die run-qemu.sh prueft.
    klog!(
        "abi",
        "Handle-Negativtest: {}/3 abgewiesen (ungueltiger Index {}, veraltete Generation {}, fehlendes Recht {})",
        case1 as usize + case2 as usize + case3 as usize,
        ergebnis(case1),
        ergebnis(case2),
        ergebnis(case3)
    );
    klog!(
        "abi",
        "Capability-Negativtest: {}/{} abgewiesen — {} {}, {} {}, {} {}, {} {}, {} {}",
        ok,
        faelle.len(),
        faelle[3].0,
        ergebnis(case4),
        faelle[4].0,
        ergebnis(case5),
        faelle[5].0,
        ergebnis(case6),
        faelle[6].0,
        ergebnis(case7),
        faelle[7].0,
        ergebnis(case8)
    );
    klog!(
        "abi",
        "Capability-Negativtest: {} {}, {} ({} Treffer bei 4096 Versuchen), {} {}, {} {}",
        faelle[8].0,
        ergebnis(case9),
        faelle[9].0,
        geraten,
        faelle[10].0,
        ergebnis(case11),
        faelle[11].0,
        ergebnis(case12)
    );
    let demo = spawn_demo();
    let ports = port_selftest();
    let uebergabe_test = transfer_selftest();
    report();
    (
        ok + demo.0 + ports.0 + uebergabe_test.0,
        faelle.len() + demo.1 + ports.1 + uebergabe_test.1,
    )
}

/// Probe der Ereignisports — des Ersatzes fuer Signals.
///
/// Positiv: binden, senden, warten; Frist 0 blockiert nicht; das Ende der
/// Gegenseite und das Ende eines Prozesses kommen als Paket an.
/// Negativ: ohne `MANAGE` am Port und ohne `WAIT` am Objekt gibt es keine
/// Bindung, und auf einem Nicht-Port wird gar nicht erst gewartet.
///
/// Liefert `(bestanden, gesamt)`.
pub fn port_selftest() -> (usize, usize) {
    let root = current();
    let mut paket = [0u8; PACKET_BYTES];
    let port = dispatch_kernel(Syscall::PortCreate as u64, [0; 6]);
    if port < 0 {
        klog!("abi", "Portprobe: FEHLER, Port konnte nicht angelegt werden");
        return (0, 7);
    }
    let port = Handle(port as u64);

    // 1. Leerer Port mit Frist 0: nachfragen, nicht warten.
    let leer = dispatch_kernel(
        Syscall::PortWait as u64,
        [port.0, paket.as_mut_ptr() as u64, 0, 0, 0, 0],
    ) == Error::WouldBlock.as_raw();

    // 2. Kanalende binden, senden, Paket abholen.
    let (a, b) = match channel_create(root) {
        Ok(p) => p,
        Err(_) => return (leer as usize, 7),
    };
    let gebunden = dispatch_kernel(
        Syscall::PortBind as u64,
        [port.0, b.0, 0x4b45_5931, Signals::READABLE.bits() as u64, 0, 0],
    ) == 0;
    let text = b"port";
    let _ = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [a.0, text.as_ptr() as u64, text.len() as u64, 0, 0, 0],
    );
    let geholt = dispatch_kernel(
        Syscall::PortWait as u64,
        [port.0, paket.as_mut_ptr() as u64, now_ns() + 50_000_000, 0, 0, 0],
    );
    let lesbar = Packet::from_bytes(&paket);
    let case_lesen = geholt == 1
        && matches!(lesbar, Some(p)
            if p.key == 0x4b45_5931 && p.signals == Signals::READABLE && p.kind == ObjectKind::Channel);

    // 3. Negativ: binden ohne MANAGE am Port.
    let schwach = dispatch_kernel(
        Syscall::HandleDuplicate as u64,
        [port.0, (Rights::WAIT | Rights::READ).bits() as u64, 0, 0, 0, 0],
    );
    let case_ohne_manage = schwach >= 0
        && dispatch_kernel(
            Syscall::PortBind as u64,
            [schwach as u64, b.0, 1, Signals::ALL.bits() as u64, 0, 0],
        ) == Error::RightsDenied.as_raw();

    // 4. Negativ: Objekt ohne WAIT-Recht laesst sich nicht beobachten.
    let stumm = grant_console(root, Rights::WRITE).unwrap_or(Handle(0));
    let case_ohne_wait = dispatch_kernel(
        Syscall::PortBind as u64,
        [port.0, stumm.0, 2, Signals::ALL.bits() as u64, 0, 0],
    ) == Error::RightsDenied.as_raw();

    // 5. Negativ: auf etwas warten, das kein Port ist.
    let case_kein_port = dispatch_kernel(
        Syscall::PortWait as u64,
        [b.0, paket.as_mut_ptr() as u64, 0, 0, 0, 0],
    ) == Error::WrongType.as_raw();

    // 6. Das Ende der Gegenseite kommt als Ereignis an.
    let _ = dispatch_kernel(
        Syscall::PortBind as u64,
        [port.0, b.0, 0x4b45_5932, Signals::PEER_CLOSED.bits() as u64, 0, 0],
    );
    let _ = dispatch_kernel(Syscall::HandleClose as u64, [a.0, 0, 0, 0, 0, 0]);
    // Das zuerst gemeldete READABLE-Paket wegraeumen, dann das gesuchte holen.
    let mut case_peer = false;
    for _ in 0..4 {
        if dispatch_kernel(
            Syscall::PortWait as u64,
            [port.0, paket.as_mut_ptr() as u64, 0, 0, 0, 0],
        ) != 1
        {
            break;
        }
        if matches!(Packet::from_bytes(&paket), Some(p)
            if p.key == 0x4b45_5932 && p.signals == Signals::PEER_CLOSED)
        {
            case_peer = true;
            break;
        }
    }

    let faelle = [
        ("Frist 0 blockiert nicht", leer),
        ("Bindung eingerichtet", gebunden),
        ("Ereignis zugestellt", case_lesen),
        ("binden ohne MANAGE abgewiesen", case_ohne_manage),
        ("beobachten ohne WAIT abgewiesen", case_ohne_wait),
        ("warten auf Nicht-Port abgewiesen", case_kein_port),
    ];
    let ok = faelle.iter().filter(|(_, b)| *b).count() + case_peer as usize;
    klog!(
        "abi",
        "Portprobe (Signals-Ersatz): {}/{} — {} {}, {} {}, {} {}, Ende der Gegenseite {}",
        ok,
        faelle.len() + 1,
        faelle[1].0,
        ergebnis(gebunden),
        faelle[2].0,
        ergebnis(case_lesen),
        faelle[0].0,
        ergebnis(leer),
        ergebnis(case_peer)
    );
    klog!(
        "abi",
        "Port-Negativtest: {} {}, {} {}, {} {}",
        faelle[3].0,
        ergebnis(case_ohne_manage),
        faelle[4].0,
        ergebnis(case_ohne_wait),
        faelle[5].0,
        ergebnis(case_kein_port)
    );
    let _ = dispatch_kernel(Syscall::HandleClose as u64, [port.0, 0, 0, 0, 0, 0]);
    (ok, faelle.len() + 1)
}

/// Probe der Handle-Uebergabe ueber einen Kanal: Weitergeben ist ein UMZUG,
/// keine Kopie — der Sender verliert das Handle, der Empfaenger bekommt einen
/// eigenen, neuen Wert. Ohne `TRANSFER` geht gar nichts.
///
/// Liefert `(bestanden, gesamt)`.
pub fn transfer_selftest() -> (usize, usize) {
    let root = current();
    let Ok((eltern, kind_ende)) = channel_create(root) else {
        klog!("abi", "Uebergabeprobe: FEHLER, Kanal fehlt");
        return (0, 5);
    };
    let Ok(kind) = spawn_with(root, "empfaenger", false, &[(kind_ende, Rights::READ)]) else {
        klog!("abi", "Uebergabeprobe: FEHLER, spawn abgewiesen");
        return (0, 5);
    };
    let empfangsende = kind_handle(kind).unwrap_or(Handle(0));

    // Ein Ausgabehandle, das der Erzeuger weitergibt.
    let gabe = grant_console(root, Rights::WRITE | Rights::TRANSFER).unwrap_or(Handle(0));
    let liste = [gabe.0];
    let text = b"handle unterwegs";
    let gesendet = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [
            eltern.0,
            text.as_ptr() as u64,
            text.len() as u64,
            liste.as_ptr() as u64,
            1,
            0,
        ],
    );
    // 1. Beim Sender ist das Handle nach dem Umzug weg.
    let case_weg = gesendet == text.len() as i64
        && dispatch_kernel(Syscall::HandleWrite as u64, [gabe.0, 0, 0, 0, 0, 0])
            == Error::BadHandle.as_raw();

    // 2. Negativ: zu wenig Platz fuer die mitgeschickten Handles — die
    //    Nachricht bleibt liegen, statt dass ein Handle verschwindet.
    let mut puffer = [0u8; 32];
    let mut aus = [0u64; 2];
    let vorher = set_current(kind);
    let ohne_platz = dispatch_kernel(
        Syscall::ChannelRecv as u64,
        [empfangsende.0, puffer.as_mut_ptr() as u64, puffer.len() as u64, 0, 0, 0],
    ) == Error::InvalidArgs.as_raw();
    // 3. Mit Platz kommt alles an — und das neue Handle traegt einen ANDEREN
    //    Wert als beim Sender.
    let gelesen = dispatch_kernel(
        Syscall::ChannelRecv as u64,
        [
            empfangsende.0,
            puffer.as_mut_ptr() as u64,
            puffer.len() as u64,
            aus.as_mut_ptr() as u64,
            aus.len() as u64,
            0,
        ],
    );
    let neu = Handle(aus[0]);
    let benutzbar = dispatch_kernel(
        Syscall::HandleWrite as u64,
        [neu.0, text.as_ptr() as u64, text.len() as u64, 0, 0, 0],
    );
    set_current(vorher);
    let case_umzug = gelesen == text.len() as i64
        && neu.is_valid()
        && neu != gabe
        && benutzbar == text.len() as i64;

    // 4. Negativ: ein Handle ohne TRANSFER-Recht laesst sich nicht mitgeben,
    //    und es bleibt danach beim Sender gueltig.
    let fest = grant_console(root, Rights::WRITE).unwrap_or(Handle(0));
    let liste2 = [fest.0];
    let verweigert = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [eltern.0, text.as_ptr() as u64, 4, liste2.as_ptr() as u64, 1, 0],
    ) == Error::RightsDenied.as_raw();
    let case_ohne_transfer =
        verweigert && dispatch_kernel(Syscall::HandleWrite as u64, [fest.0, 0, 0, 0, 0, 0]) == 0;

    // 5. Negativ: dasselbe Handle zweimal in einer Nachricht.
    let doppelt = grant_console(root, Rights::WRITE | Rights::TRANSFER).unwrap_or(Handle(0));
    let liste3 = [doppelt.0, doppelt.0];
    let case_doppelt = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [eltern.0, text.as_ptr() as u64, 4, liste3.as_ptr() as u64, 2, 0],
    ) == Error::InvalidArgs.as_raw()
        && dispatch_kernel(Syscall::HandleWrite as u64, [doppelt.0, 0, 0, 0, 0, 0]) == 0;

    let ok = case_weg as usize
        + case_umzug as usize
        + case_ohne_transfer as usize
        + case_doppelt as usize
        + ohne_platz as usize;
    klog!(
        "abi",
        "Uebergabeprobe: Handle per Kanal umgezogen {} (Sender {:#x} -> Empfaenger {:#x}), beim Sender ungueltig {}, ohne Platz keine Zustellung {}",
        ergebnis(case_umzug),
        gabe.0,
        neu.0,
        ergebnis(case_weg),
        ergebnis(ohne_platz)
    );
    klog!(
        "abi",
        "Uebergabe-Negativtest: ohne TRANSFER-Recht abgewiesen {}, dasselbe Handle doppelt abgewiesen {}",
        ergebnis(case_ohne_transfer),
        ergebnis(case_doppelt)
    );
    let _ = exit_process(kind, 0);
    (ok, 5)
}

/// Nachschlagen eines Handles mit Rechteprobe — ohne Nebenwirkung.
fn lookup(pid: u32, h: Handle, needed: Rights) -> Result<HandleEntry> {
    let n = lock();
    let p = n.procs.get(pid as usize).ok_or(Error::NotFound)?;
    p.handles.get_with(h, needed).copied()
}

/// Positivprobe: zwei Prozesse, ein Kanal, ein explizit uebergebenes Handle.
/// Zeigt im Log, dass der spawn-Stil trägt — Rechte werden dabei verkleinert.
///
/// Liefert `(bestanden, gesamt)`.
pub fn spawn_demo() -> (usize, usize) {
    let root = current();
    let mut ok = 0usize;
    let gesamt = 3usize;

    // Kanal im Erzeuger anlegen, ein Ende dem Kind geben — mit weniger Rechten.
    let Ok((eltern, kind_ende)) = channel_create(root) else {
        klog!("abi", "Prozessprobe: Kanal konnte nicht angelegt werden");
        return (0, gesamt);
    };
    let kind = match spawn_with(root, "dienst", false, &[(kind_ende, Rights::READ)]) {
        Ok(p) => p,
        Err(e) => {
            klog!("abi", "Prozessprobe: spawn abgewiesen ({})", e.name());
            return (0, gesamt);
        }
    };
    // 1. Das Kind hat GENAU ein Handle — kein Erben, kein Standardsatz.
    if handle_count(kind) == 1 {
        ok += 1;
    }
    // 2. Nachricht vom Erzeuger an das Kind.
    let text = b"hallo aus dem Erzeuger";
    let gesendet = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [eltern.0, text.as_ptr() as u64, text.len() as u64, 0, 0, 0],
    );
    let mut puffer = [0u8; 64];
    let vorher = set_current(kind);
    let empfangen = dispatch_kernel(
        Syscall::ChannelRecv as u64,
        [
            kind_handle(kind).map(|h| h.0).unwrap_or(0),
            puffer.as_mut_ptr() as u64,
            puffer.len() as u64,
            0,
            0,
            0,
        ],
    );
    // 3. Das Kind darf auf seinem Ende NICHT schreiben (Recht abgegeben).
    let schreibversuch = dispatch_kernel(
        Syscall::ChannelSend as u64,
        [kind_handle(kind).map(|h| h.0).unwrap_or(0), text.as_ptr() as u64, 4, 0, 0, 0],
    );
    set_current(vorher);
    if gesendet == text.len() as i64 && empfangen == text.len() as i64 {
        ok += 1;
    }
    if schreibversuch == Error::RightsDenied.as_raw() {
        ok += 1;
    }
    let inhalt = core::str::from_utf8(&puffer[..(empfangen.max(0) as usize).min(puffer.len())])
        .unwrap_or("<nicht darstellbar>");
    klog!(
        "abi",
        "Prozessprobe: spawn ohne fork, Kind \"dienst\" mit {} explizit uebergebenen Handle(n); Kanal: {} B gesendet, {} B empfangen (\"{}\"), Schreiben ohne Recht -> {}",
        handle_count(kind),
        gesendet.max(0),
        empfangen.max(0),
        inhalt,
        if schreibversuch == Error::RightsDenied.as_raw() {
            Error::RightsDenied.name()
        } else {
            "FEHLER: durchgelassen"
        }
    );
    let _ = exit_process(kind, 0);
    (ok, gesamt)
}

/// Erstes Handle eines Prozesses (die Kinder dieser Probe haben genau eines).
fn kind_handle(pid: u32) -> Option<Handle> {
    let n = lock();
    let h = n.procs.get(pid as usize)?.handles.iter().next().map(|e| e.handle);
    h
}

/// Kurzer Selbsttest der ABI-Kernelseite fuer das Boot-Log.
/// Liefert (ABI-Version, Handles des Wurzelprozesses, Fehlercode eines
/// absichtlich ungueltigen Handles).
pub fn self_test() -> (i64, usize, &'static str) {
    let version = dispatch_kernel(Syscall::Version as u64, [0; 6]);
    let root = current();
    let h = grant_console(root, Rights::READ | Rights::WRITE).unwrap_or(Handle(0));
    let stale = h;
    let _ = dispatch_kernel(Syscall::HandleClose as u64, [h.0, 0, 0, 0, 0, 0]);
    let err = match lookup(root, stale, Rights::READ) {
        Err(e) => e.name(),
        Ok(_) => "FEHLER: geschlossenes Handle noch gueltig",
    };
    (version, handle_count(root), err)
}

/// Hilfsanzeige: alle Prozessnamen in einer Zeile, ohne Zwischenpuffer.
struct NameListe<'a>(&'a [Process]);

impl core::fmt::Display for NameListe<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, p) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}:{}", p.pid, p.name)?;
        }
        Ok(())
    }
}

/// Zaehlerstand der ABI fuer das Boot-Log.
pub fn report() {
    let n = lock();
    klog!(
        "abi",
        "Prozesse    : {} in der Tafel, {} per spawn erzeugt, {} Aufrufe, {} abgewiesen (davon {} aus unprivilegierten Prozessen), {} Objekte",
        n.procs.len(),
        n.spawns,
        n.syscalls,
        n.denied,
        n.user_denied,
        n.objects.iter().filter(|o| !matches!(o.data, ObjectData::Free)).count()
    );
    let speicher: u64 = n
        .objects
        .iter()
        .map(|o| match o.data {
            ObjectData::Memory { len } => len,
            _ => 0,
        })
        .sum();
    klog!(
        "abi",
        "Prozesse    : {}, davon beendet {}, Speicherobjekte {} B; Namen: {}",
        n.procs.len(),
        n.procs.iter().filter(|p| p.exit_code.is_some()).count(),
        speicher,
        NameListe(&n.procs)
    );
}
