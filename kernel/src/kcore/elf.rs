//! ELF64-Lader — Pruefteil (architekturneutral).
//!
//! **Schicht: core.** Diese Datei versteht das Dateiformat und sonst nichts:
//! sie prueft den Kopf, laeuft die Programmkoepfe ab und liefert
//! Ladeauftraege. Das eigentliche Abbilden der Segmente macht `mm` ueber die
//! Adressraum-Abstraktion — hier kommt kein Hardwarebit vor.
//!
//! **Grundsatz: Muell fuehrt zu einem Fehlerwert, nie zu einem Absturz.**
//! Jede Laengen- und Bereichspruefung passiert VOR dem ersten Zugriff; alle
//! Additionen sind ueberlaufsicher (`checked_add`).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::AddressSpace;
use crate::kcore::arch_iface::AddressSpaceOps;
use crate::kcore::mem::{phys_to_virt, MapError, MapFlags, PhysAddr, VirtAddr, PAGE_SIZE};
use crate::kcore::user::{is_user_addr, USER_LIMIT};
use crate::klog;

/// Warum ein Abbild nicht geladen werden kann.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElfError {
    /// Datei kuerzer als der Kopf.
    TooShort,
    /// Kennung am Dateianfang stimmt nicht.
    BadMagic,
    /// Keine 64-Bit-Datei.
    BadClass,
    /// Falsche Bytereihenfolge.
    BadEndian,
    /// Kein ausfuehrbares Abbild (z. B. Objektdatei oder dynamisch gebunden).
    BadType,
    /// Falsche Zielarchitektur.
    BadMachine,
    /// Tabelle der Programmkoepfe liegt ausserhalb der Datei.
    BadTable,
    /// Ein Segment liegt ausserhalb der Datei.
    SegmentOutOfFile,
    /// Ein Segment liegt ausserhalb des unprivilegierten Adressbereichs.
    SegmentOutOfRange,
    /// Zwei Segmente ueberlappen sich.
    SegmentOverlap,
    /// Segmentangaben sind in sich unstimmig (z. B. Dateilaenge > Speicherlaenge).
    SegmentInsane,
    /// Ausrichtungsangabe des Segments ist keine Zweierpotenz oder Datei- und
    /// Speicherlage passen modulo dieser Ausrichtung nicht zueinander.
    BadAlign,
    /// Das Abbild verlangt einen Binder (`PT_INTERP`) — der Kern bindet nicht.
    NeedsInterpreter,
    /// Einsprungadresse liegt in keinem geladenen Segment.
    BadEntry,
    /// Mehr Segmente, als der Lader annimmt.
    TooManySegments,
}

impl ElfError {
    /// Klartext fuer das Boot-Log.
    pub const fn describe(self) -> &'static str {
        match self {
            ElfError::TooShort => "Datei kuerzer als der Kopf",
            ElfError::BadMagic => "falsche Kennung am Dateianfang",
            ElfError::BadClass => "keine 64-Bit-Datei",
            ElfError::BadEndian => "falsche Bytereihenfolge",
            ElfError::BadType => "kein statisch gebundenes ausfuehrbares Abbild",
            ElfError::BadMachine => "falsche Zielarchitektur",
            ElfError::BadTable => "Programmkopftabelle ausserhalb der Datei",
            ElfError::SegmentOutOfFile => "Segment ausserhalb der Datei",
            ElfError::SegmentOutOfRange => "Segment ausserhalb des unprivilegierten Bereichs",
            ElfError::SegmentOverlap => "Segmente ueberlappen sich",
            ElfError::SegmentInsane => "unstimmige Segmentangaben",
            ElfError::BadAlign => "unstimmige Ausrichtungsangabe",
            ElfError::NeedsInterpreter => "dynamisch gebunden, verlangt einen Binder",
            ElfError::BadEntry => "Einsprungadresse in keinem Segment",
            ElfError::TooManySegments => "zu viele Segmente",
        }
    }
}

/// Hoechstzahl der Segmente, die der Lader annimmt.
pub const MAX_SEGMENTS: usize = 16;

/// Ein zu ladendes Segment, bereits geprueft.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Segment {
    /// Zieladresse im unprivilegierten Adressraum.
    pub vaddr: u64,
    /// Anzahl Bytes, die aus der Datei kopiert werden.
    pub file_len: u64,
    /// Anzahl Bytes, die im Speicher belegt werden (Rest wird genullt).
    pub mem_len: u64,
    /// Beginn der Daten in der Datei.
    pub file_off: u64,
    /// Lesbar.
    pub read: bool,
    /// Beschreibbar.
    pub write: bool,
    /// Ausfuehrbar.
    pub exec: bool,
}

impl Segment {
    /// Kurzschreibweise der Rechte fuer Log-Zeilen.
    pub const fn rwx(&self) -> &'static str {
        match (self.read, self.write, self.exec) {
            (_, true, true) => "RWX",
            (_, true, false) => "RW",
            (_, false, true) => "RX",
            _ => "R",
        }
    }
}

/// Ein geprueftes Abbild: Einsprung plus Segmentliste.
#[derive(Clone, Copy)]
pub struct Image {
    /// Einsprungadresse.
    pub entry: u64,
    /// Geprueft uebernommene Segmente.
    pub segments: [Segment; MAX_SEGMENTS],
    /// Anzahl belegter Eintraege in [`Image::segments`].
    pub count: usize,
}

impl Image {
    /// Die geladenen Segmente.
    pub fn segments(&self) -> &[Segment] {
        &self.segments[..self.count]
    }
}

const EI_NIDENT: usize = 16;
const EHDR_LEN: usize = 64;
const PHDR_LEN: usize = 56;
const PT_LOAD: u32 = 1;
/// Verweis auf einen Binder. Kommt nur in dynamisch gebundenen Abbildern vor;
/// dieser Kern bindet nicht und weist solche Abbilder ab, statt sie halb zu
/// laden und beim ersten Sprung ins Leere laufen zu lassen.
const PT_INTERP: u32 = 3;
const ET_EXEC: u16 = 2;
/// Maschinenkennung der aktiven Architektur (x86_64 = 62).
const EM_EXPECTED: u16 = 62;

fn rd16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn rd32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn rd64(b: &[u8], off: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[off..off + 8]);
    u64::from_le_bytes(v)
}

/// Prueft ein Abbild vollstaendig durch und liefert die Ladeauftraege.
///
/// Fuehrt KEINE Speicherzugriffe ausserhalb von `bytes` aus und bildet nichts
/// ab — deshalb ist die Funktion sicher und host-testbar.
pub fn parse(bytes: &[u8]) -> Result<Image, ElfError> {
    if bytes.len() < EHDR_LEN {
        return Err(ElfError::TooShort);
    }
    let id = &bytes[..EI_NIDENT];
    if id[0] != 0x7f || id[1] != b'E' || id[2] != b'L' || id[3] != b'F' {
        return Err(ElfError::BadMagic);
    }
    if id[4] != 2 {
        return Err(ElfError::BadClass);
    }
    if id[5] != 1 {
        return Err(ElfError::BadEndian);
    }
    if rd16(bytes, 16) != ET_EXEC {
        return Err(ElfError::BadType);
    }
    if rd16(bytes, 18) != EM_EXPECTED {
        return Err(ElfError::BadMachine);
    }
    let entry = rd64(bytes, 24);
    let phoff = rd64(bytes, 32);
    let phentsize = rd16(bytes, 54) as usize;
    let phnum = rd16(bytes, 56) as usize;
    if phentsize != PHDR_LEN || phnum == 0 {
        return Err(ElfError::BadTable);
    }
    let table_len = phnum.checked_mul(PHDR_LEN).ok_or(ElfError::BadTable)?;
    let start = usize::try_from(phoff).map_err(|_| ElfError::BadTable)?;
    let end = start.checked_add(table_len).ok_or(ElfError::BadTable)?;
    if end > bytes.len() {
        return Err(ElfError::BadTable);
    }

    let mut img = Image { entry, segments: [Segment::default(); MAX_SEGMENTS], count: 0 };
    for i in 0..phnum {
        let p = start + i * PHDR_LEN;
        let ptype = rd32(bytes, p);
        if ptype == PT_INTERP {
            return Err(ElfError::NeedsInterpreter);
        }
        if ptype != PT_LOAD {
            continue;
        }
        if img.count == MAX_SEGMENTS {
            return Err(ElfError::TooManySegments);
        }
        let flags = rd32(bytes, p + 4);
        let off = rd64(bytes, p + 8);
        let vaddr = rd64(bytes, p + 16);
        let filesz = rd64(bytes, p + 32);
        let memsz = rd64(bytes, p + 40);
        let align = rd64(bytes, p + 48);
        if filesz > memsz || memsz == 0 {
            return Err(ElfError::SegmentInsane);
        }
        // Ausrichtung: entweder ohne Bedingung (0 oder 1) oder eine
        // Zweierpotenz, und dann muessen Datei- und Speicherlage modulo dieser
        // Zahl uebereinstimmen. Stimmt das nicht, kann der Lader den Inhalt
        // gar nicht seitenweise an die richtige Stelle kopieren — so ein
        // Abbild ist in sich falsch, nicht bloss unbequem.
        if align > 1 {
            if !align.is_power_of_two() || vaddr % align != off % align {
                return Err(ElfError::BadAlign);
            }
        }
        let fend = off.checked_add(filesz).ok_or(ElfError::SegmentOutOfFile)?;
        if fend > bytes.len() as u64 {
            return Err(ElfError::SegmentOutOfFile);
        }
        let vend = vaddr.checked_add(memsz).ok_or(ElfError::SegmentOutOfRange)?;
        if !is_user_addr(vaddr) || vend > USER_LIMIT {
            return Err(ElfError::SegmentOutOfRange);
        }
        let seg = Segment {
            vaddr,
            file_len: filesz,
            mem_len: memsz,
            file_off: off,
            read: flags & 4 != 0,
            write: flags & 2 != 0,
            exec: flags & 1 != 0,
        };
        for prev in img.segments().iter() {
            let pend = prev.vaddr + prev.mem_len;
            if vaddr < pend && prev.vaddr < vend {
                return Err(ElfError::SegmentOverlap);
            }
        }
        img.segments[img.count] = seg;
        img.count += 1;
    }
    if img.count == 0 {
        return Err(ElfError::BadTable);
    }
    let ok = img.segments().iter().any(|s| entry >= s.vaddr && entry < s.vaddr + s.mem_len && s.exec);
    if !ok {
        return Err(ElfError::BadEntry);
    }
    Ok(img)
}

// ---------------------------------------------------------------- Laden
//
// Ab hier wird wirklich abgebildet. Auch dieser Teil bleibt architekturneutral:
// er kennt nur [`AddressSpaceOps`], [`MapFlags`] und den Frame-Allocator aus
// `mm` — kein Tabellenbit, kein Register.

/// Oberes Ende des unprivilegierten Stapels (waechst nach unten).
pub const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;
/// Groesse des unprivilegierten Stapels in Basisseiten.
pub const USER_STACK_PAGES: u64 = 4;
/// Hoechstzahl Seiten, die ein Abbild belegen darf. Begrenzt gleichzeitig den
/// Rueckrollpuffer: schlaegt eine Abbildung mittendrin fehl, wird jede bereits
/// angelegte Seite wieder entfernt und ihr Frame zurueckgegeben.
pub const MAX_IMAGE_PAGES: usize = 256;

/// Warum ein Abbild nicht in einen Adressraum gebracht werden konnte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoadError {
    /// Das Abbild selbst ist unbrauchbar.
    Format(ElfError),
    /// Der Adressraum liess die Abbildung nicht zu.
    Map(MapError),
    /// Kein Frame-Allocator verfuegbar.
    NoAllocator,
    /// Kein physischer Speicher mehr.
    OutOfFrames,
    /// Segment gleichzeitig beschreib- und ausfuehrbar (W^X).
    WriteAndExec,
    /// Abbild braucht mehr Seiten als [`MAX_IMAGE_PAGES`].
    TooLarge,
}

impl LoadError {
    /// Klartext fuer das Boot-Log.
    pub fn describe(self) -> &'static str {
        match self {
            LoadError::Format(e) => e.describe(),
            LoadError::Map(e) => e.describe(),
            LoadError::NoAllocator => "kein Frame-Allocator verfuegbar",
            LoadError::OutOfFrames => "kein physischer Speicher mehr",
            LoadError::WriteAndExec => "Segment beschreibbar UND ausfuehrbar (W^X verletzt)",
            LoadError::TooLarge => "Abbild belegt zu viele Seiten",
        }
    }
}

/// Ein fertig geladenes Programm.
#[derive(Clone, Copy)]
pub struct Loaded {
    /// Einsprungadresse.
    pub entry: VirtAddr,
    /// Oberes Ende des unprivilegierten Stapels.
    pub stack_top: VirtAddr,
    /// Anzahl abgebildeter Seiten (Programm ohne Stapel).
    pub pages: usize,
    /// Anzahl geladener Segmente.
    pub segments: usize,
}

/// Merkzettel fuer das Rueckrollen halb angelegter Abbildungen.
struct Placed {
    pages: [u64; MAX_IMAGE_PAGES],
    count: usize,
}

impl Placed {
    fn new() -> Self {
        Placed { pages: [0; MAX_IMAGE_PAGES], count: 0 }
    }

    fn push(&mut self, v: u64) -> Result<(), LoadError> {
        if self.count == MAX_IMAGE_PAGES {
            return Err(LoadError::TooLarge);
        }
        self.pages[self.count] = v;
        self.count += 1;
        Ok(())
    }

    /// Entfernt alle bereits angelegten Seiten wieder und gibt ihre Frames
    /// zurueck. Ohne das haette ein halb geladenes Muellabbild Speicher und
    /// Adressraum dauerhaft belegt.
    unsafe fn rollback(&self, space: &mut AddressSpace) {
        let mut guard = crate::mm::frame::lock();
        let Some(fa) = guard.as_mut() else { return };
        for i in 0..self.count {
            if let Ok(p) = unsafe { space.unmap(VirtAddr(self.pages[i])) } {
                crate::kcore::mem::FrameAllocator::free_frame(fa, p);
            }
        }
    }
}

/// Legt eine genullte, unprivilegiert erreichbare Seite an und liefert ihre
/// physische Adresse.
unsafe fn place_page(
    space: &mut AddressSpace,
    virt: u64,
    flags: MapFlags,
) -> Result<PhysAddr, LoadError> {
    let mut guard = crate::mm::frame::lock();
    let fa = guard.as_mut().ok_or(LoadError::NoAllocator)?;
    let frame = crate::kcore::mem::FrameAllocator::alloc_frame(fa)
        .ok_or(LoadError::OutOfFrames)?;
    match unsafe { space.map(VirtAddr(virt), frame, flags, fa) } {
        Ok(()) => Ok(frame),
        Err(e) => {
            crate::kcore::mem::FrameAllocator::free_frame(fa, frame);
            Err(LoadError::Map(e))
        }
    }
}

/// Bildet ein geprueftes Abbild in `space` ab und legt einen unprivilegierten
/// Stapel an.
///
/// Die Segmentinhalte werden ueber das Direct-Map-Fenster geschrieben, nicht
/// ueber die neue Abbildung: nur so laesst sich ein nur lesbares Codesegment
/// befuellen, ohne es voruebergehend beschreibbar zu machen.
///
/// # Safety
/// `space` muss der aktive oder ein vollstaendig aufgebauter Adressraum sein;
/// der unprivilegierte Bereich darin muss frei sein.
pub unsafe fn load(space: &mut AddressSpace, bytes: &[u8]) -> Result<Loaded, LoadError> {
    let img = parse(bytes).map_err(LoadError::Format)?;
    let mut placed = Placed::new();
    let page = PAGE_SIZE as u64;

    for seg in img.segments() {
        if seg.write && seg.exec {
            unsafe { placed.rollback(space) };
            return Err(LoadError::WriteAndExec);
        }
        let mut flags = MapFlags::USER | MapFlags::READ;
        if seg.write {
            flags = flags | MapFlags::WRITE;
        }
        if seg.exec {
            flags = flags | MapFlags::EXEC;
        }
        let start = seg.vaddr & !(page - 1);
        let end = (seg.vaddr + seg.mem_len + page - 1) & !(page - 1);
        let mut v = start;
        while v < end {
            let frame = match unsafe { place_page(space, v, flags) } {
                Ok(f) => f,
                Err(e) => {
                    unsafe { placed.rollback(space) };
                    return Err(e);
                }
            };
            if let Err(e) = placed.push(v) {
                unsafe { placed.rollback(space) };
                return Err(e);
            }
            // Anteil dieser Seite, der aus der Datei kommt. Alles andere
            // bleibt genullt — das ist die .bss.
            let page_start = v;
            let page_end = v + page;
            let data_start = seg.vaddr;
            let data_end = seg.vaddr + seg.file_len;
            let from = data_start.max(page_start);
            let to = data_end.min(page_end);
            if from < to {
                let dst = phys_to_virt(frame).as_ptr::<u8>();
                let src_off = (seg.file_off + (from - data_start)) as usize;
                let len = (to - from) as usize;
                // SAFETY: `parse` hat bereits geprueft, dass
                // [file_off, file_off+file_len) in `bytes` liegt; Ziel ist
                // eine frisch belegte Seite im Direct-Map-Fenster.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        bytes.as_ptr().add(src_off),
                        dst.add((from - page_start) as usize),
                        len,
                    )
                };
            }
            v += page;
        }
    }

    // Unprivilegierter Stapel.
    let stack_flags = MapFlags::USER | MapFlags::READ | MapFlags::WRITE;
    let mut i = 0;
    while i < USER_STACK_PAGES {
        let v = USER_STACK_TOP - (i + 1) * page;
        if let Err(e) = unsafe { place_page(space, v, stack_flags) } {
            unsafe { placed.rollback(space) };
            return Err(e);
        }
        if let Err(e) = placed.push(v) {
            unsafe { placed.rollback(space) };
            return Err(e);
        }
        i += 1;
    }

    Ok(Loaded {
        entry: VirtAddr(img.entry),
        stack_top: VirtAddr(USER_STACK_TOP),
        pages: placed.count,
        segments: img.count,
    })
}

/// Wie weit ein geladenes Abbild wieder abgeraeumt wird.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unload {
    /// Nur die Programmsegmente. Der unprivilegierte Stapel bleibt stehen —
    /// das ist der Fall, wenn ein weiteres Abbild in denselben Adressraum
    /// nachgeladen wird und den Stapel weiterbenutzt.
    SegmentsOnly,
    /// Segmente **und** unprivilegierter Stapel. Danach ist der
    /// unprivilegierte Teil des Adressraums wieder leer.
    WithStack,
}

/// Nimmt ein geladenes Abbild wieder aus `space` heraus und gibt jeden Frame
/// an den Allocator zurueck.
///
/// Gerechnet wird aus der Datei, nicht aus einem mitgefuehrten Merkzettel:
/// dieselbe Pruefung wie beim Laden liefert dieselben Seitenbereiche. Seiten,
/// die gar nicht (mehr) abgebildet sind, werden uebergangen — das Abraeumen
/// darf nie selbst zum Fehlerfall werden.
///
/// Liefert die Anzahl zurueckgegebener Seiten.
///
/// # Safety
/// `space` muss gueltig sein; in den freigegebenen Seiten darf niemand mehr
/// arbeiten (das Programm ist beendet).
pub unsafe fn unload(
    space: &mut AddressSpace,
    bytes: &[u8],
    scope: Unload,
) -> Result<usize, LoadError> {
    let img = parse(bytes).map_err(LoadError::Format)?;
    let page = PAGE_SIZE as u64;
    let mut freed = 0usize;
    let mut guard = crate::mm::frame::lock();
    let fa = guard.as_mut().ok_or(LoadError::NoAllocator)?;
    for seg in img.segments() {
        let start = seg.vaddr & !(page - 1);
        let end = (seg.vaddr + seg.mem_len + page - 1) & !(page - 1);
        let mut v = start;
        while v < end {
            if let Ok(p) = unsafe { space.unmap(VirtAddr(v)) } {
                crate::kcore::mem::FrameAllocator::free_frame(fa, p);
                freed += 1;
            }
            v += page;
        }
    }
    if scope == Unload::WithStack {
        let mut i = 0;
        while i < USER_STACK_PAGES {
            let v = USER_STACK_TOP - (i + 1) * page;
            if let Ok(p) = unsafe { space.unmap(VirtAddr(v)) } {
                crate::kcore::mem::FrameAllocator::free_frame(fa, p);
                freed += 1;
            }
            i += 1;
        }
    }
    Ok(freed)
}

/// Einsprung und Stapel des zuletzt geladenen Programms — 0, solange keines
/// geladen wurde.
static PROGRAM_ENTRY: AtomicU64 = AtomicU64::new(0);
static PROGRAM_STACK: AtomicU64 = AtomicU64::new(0);

/// Das geladene unprivilegierte Programm (Einsprung, Stapelobergrenze).
///
/// Damit kann die unprivilegierte Ebene starten, was der Lader aus dem
/// Startdateisystem geholt hat — ohne dass sie das Dateiformat oder das
/// Archiv kennen muss.
pub fn program() -> Option<(VirtAddr, VirtAddr)> {
    let e = PROGRAM_ENTRY.load(Ordering::SeqCst);
    let s = PROGRAM_STACK.load(Ordering::SeqCst);
    if e == 0 || s == 0 {
        None
    } else {
        Some((VirtAddr(e), VirtAddr(s)))
    }
}

/// Laedt `name` aus dem Startdateisystem in `space`, meldet das Ergebnis ins
/// Boot-Log und merkt sich Einsprung und Stapel fuer [`program`].
///
/// # Safety
/// Wie [`load`].
pub unsafe fn load_from_initramfs(
    space: &mut AddressSpace,
    name: &str,
) -> Result<Loaded, LoadError> {
    let Some(bytes) = crate::kcore::initramfs::find(name) else {
        klog!("elf", "ELF-Lader   : {} nicht im Archiv", name);
        return Err(LoadError::Format(ElfError::TooShort));
    };
    let img = parse(bytes).map_err(LoadError::Format)?;
    let loaded = unsafe { load(space, bytes) }?;
    klog!(
        "elf",
        "ELF-Lader   : {}, {} Segmente geladen, Einsprung {:#018x}",
        name,
        loaded.segments,
        loaded.entry.as_u64()
    );
    for (i, s) in img.segments().iter().enumerate() {
        klog!(
            "elf",
            "  Segment {}: {:#x} {} KiB {} ({} B aus der Datei, {} B genullt)",
            i,
            s.vaddr,
            (s.mem_len + 1023) / 1024,
            s.rwx(),
            s.file_len,
            s.mem_len - s.file_len
        );
    }
    klog!(
        "elf",
        "  Stapel: {:#018x} abwaerts, {} Seiten, insgesamt {} Seiten abgebildet",
        loaded.stack_top.as_u64(),
        USER_STACK_PAGES,
        loaded.pages
    );
    PROGRAM_ENTRY.store(loaded.entry.as_u64(), Ordering::SeqCst);
    PROGRAM_STACK.store(loaded.stack_top.as_u64(), Ordering::SeqCst);
    Ok(loaded)
}

/// Prueft nach dem Laden, dass die Abbildung wirklich steht: Einsprungseite
/// vorhanden, Stapelseite vorhanden, und am Einsprung stehen genau die Bytes
/// aus der Datei.
fn verify_loaded(space: &AddressSpace, bytes: &[u8], l: &Loaded) -> bool {
    let Some(phys) = space.translate(l.entry) else { return false };
    let Some(_) = space.translate(VirtAddr(l.stack_top.as_u64() - PAGE_SIZE as u64)) else {
        return false;
    };
    let Ok(img) = parse(bytes) else { return false };
    let Some(seg) = img.segments().iter().find(|s| {
        l.entry.as_u64() >= s.vaddr && l.entry.as_u64() < s.vaddr + s.mem_len
    }) else {
        return false;
    };
    let off = (seg.file_off + (l.entry.as_u64() - seg.vaddr)) as usize;
    let n = 16.min(bytes.len() - off);
    // SAFETY: `phys` kommt aus der eben angelegten Abbildung und ist ueber das
    // Direct-Map-Fenster lesbar.
    let seen = unsafe { core::slice::from_raw_parts(phys_to_virt(phys).as_ptr::<u8>(), n) };
    seen == &bytes[off..off + n]
}

/// Baut einen ELF64-Kopf mit einem einzigen ladbaren Segment in `buf` und
/// liefert die belegte Laenge. Grundlage der Negativtests: jeder Fall
/// veraendert genau ein Feld dieses gueltigen Abbilds.
fn build_probe(buf: &mut [u8; 256]) -> usize {
    buf.fill(0);
    buf[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    buf[4] = 2; // 64 Bit
    buf[5] = 1; // little endian
    buf[6] = 1; // Version
    buf[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
    buf[18..20].copy_from_slice(&EM_EXPECTED.to_le_bytes());
    buf[24..32].copy_from_slice(&0x0040_1000u64.to_le_bytes()); // Einsprung
    buf[32..40].copy_from_slice(&(EHDR_LEN as u64).to_le_bytes()); // Tabelle
    buf[54..56].copy_from_slice(&(PHDR_LEN as u16).to_le_bytes());
    buf[56..58].copy_from_slice(&1u16.to_le_bytes());
    let p = EHDR_LEN;
    buf[p..p + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
    buf[p + 4..p + 8].copy_from_slice(&5u32.to_le_bytes()); // R+X
    buf[p + 8..p + 16].copy_from_slice(&0u64.to_le_bytes()); // Dateibeginn
    buf[p + 16..p + 24].copy_from_slice(&0x0040_1000u64.to_le_bytes()); // vaddr
    buf[p + 32..p + 40].copy_from_slice(&128u64.to_le_bytes()); // Dateilaenge
    buf[p + 40..p + 48].copy_from_slice(&0x1000u64.to_le_bytes()); // Speicherlaenge
    // Der ganze Puffer gilt als Datei — nur so liegt das 128 B lange Segment
    // wirklich innerhalb der Datei, und die Negativfaelle unterscheiden sich
    // vom gueltigen Fall in genau einem Feld.
    buf.len()
}

/// Selbsttest des Laders: ein gueltiges Abbild muss durchkommen, jede Sorte
/// Muell muss mit dem PASSENDEN Fehler abgewiesen werden — und keiner der
/// Faelle darf den Kernel anhalten.
///
/// Liefert `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    let mut buf = [0u8; 256];
    let len = build_probe(&mut buf);
    let mut ok = 0usize;
    let mut total = 0usize;
    let mut check = |name: &str, expect: Result<(), ElfError>, got: Result<Image, ElfError>| {
        total += 1;
        let good = match (expect, &got) {
            (Ok(()), Ok(_)) => true,
            (Err(e), Err(g)) => e == *g,
            _ => false,
        };
        if good {
            ok += 1;
        }
        crate::klog!(
            "elf",
            "  {:<28} -> {} {}",
            name,
            match &got {
                Ok(i) => {
                    let _ = i;
                    "angenommen"
                }
                Err(e) => e.describe(),
            },
            if good { "(ok)" } else { "(FEHLER)" }
        );
    };

    check("gueltiges Abbild", Ok(()), parse(&buf[..len]));
    check("leere Datei", Err(ElfError::TooShort), parse(&[]));

    let mut bad = buf;
    bad[1] = b'X';
    check("falsche Kennung", Err(ElfError::BadMagic), parse(&bad[..len]));

    let mut bad = buf;
    bad[4] = 1;
    check("32-Bit-Klasse", Err(ElfError::BadClass), parse(&bad[..len]));

    let mut bad = buf;
    bad[5] = 2;
    check("falsche Bytereihenfolge", Err(ElfError::BadEndian), parse(&bad[..len]));

    let mut bad = buf;
    bad[18..20].copy_from_slice(&40u16.to_le_bytes());
    check("fremde Architektur", Err(ElfError::BadMachine), parse(&bad[..len]));

    let mut bad = buf;
    bad[32..40].copy_from_slice(&0xffff_0000u64.to_le_bytes());
    check("Tabelle ausserhalb", Err(ElfError::BadTable), parse(&bad[..len]));

    let mut bad = buf;
    bad[EHDR_LEN + 32..EHDR_LEN + 40].copy_from_slice(&0x1000u64.to_le_bytes());
    check("Segment ausserhalb der Datei", Err(ElfError::SegmentOutOfFile), parse(&bad[..len]));

    let mut bad = buf;
    bad[EHDR_LEN + 16..EHDR_LEN + 24]
        .copy_from_slice(&0xffff_8000_0000_0000u64.to_le_bytes());
    check("Segment im Kernelbereich", Err(ElfError::SegmentOutOfRange), parse(&bad[..len]));

    let mut bad = buf;
    bad[EHDR_LEN + 32..EHDR_LEN + 40].copy_from_slice(&0x9000u64.to_le_bytes());
    check("Dateilaenge > Speicherlaenge", Err(ElfError::SegmentInsane), parse(&bad[..len]));

    let mut bad = buf;
    bad[EHDR_LEN + 48..EHDR_LEN + 56].copy_from_slice(&3u64.to_le_bytes());
    check("Ausrichtung krumm", Err(ElfError::BadAlign), parse(&bad[..len]));

    let mut bad = buf;
    bad[EHDR_LEN + 48..EHDR_LEN + 56].copy_from_slice(&0x200u64.to_le_bytes());
    bad[EHDR_LEN + 8..EHDR_LEN + 16].copy_from_slice(&8u64.to_le_bytes());
    check("Datei-/Speicherlage unpassend", Err(ElfError::BadAlign), parse(&bad[..len]));

    // Zwei Programmkoepfe: der zweite verlangt einen Binder. Ein solches
    // Abbild darf gar nicht erst geladen werden — der Kern bindet nicht.
    let mut bad = buf;
    bad[56..58].copy_from_slice(&2u16.to_le_bytes());
    let p2 = EHDR_LEN + PHDR_LEN;
    bad[p2..p2 + 4].copy_from_slice(&PT_INTERP.to_le_bytes());
    check("dynamisch gebunden", Err(ElfError::NeedsInterpreter), parse(&bad[..len]));

    let mut bad = buf;
    bad[24..32].copy_from_slice(&0u64.to_le_bytes());
    check("Einsprung ausserhalb", Err(ElfError::BadEntry), parse(&bad[..len]));

    crate::klog!("elf", "ELF-Negativtest: {}/{} Faelle wie erwartet abgewiesen", ok, total);

    // Der Archivleser wird an derselben Stelle mitgeprueft — beide gehoeren
    // zum selben Weg "vom Bootloader-Modul zum laufenden Programm".
    let (a_ok, a_total) = crate::kcore::initramfs::selftest();
    ok += a_ok;
    total += a_total;

    // Und jetzt der Ernstfall: echte Dateien aus dem echten Archiv.
    let (l_ok, l_total) = load_selftest();
    ok += l_ok;
    total += l_total;

    (ok, total)
}

/// Ladeversuche mit den ECHTEN Dateien aus dem Startdateisystem — ein
/// gueltiges Programm, ein absichtlich kaputtes Abbild und eine Textdatei.
///
/// Ohne Archiv (z. B. wenn der Bootloader kein Modul geliefert hat) meldet die
/// Funktion das und zaehlt nichts; der Boot bleibt in jedem Fall gruen.
fn load_selftest() -> (usize, usize) {
    let archive = match crate::kcore::initramfs::archive() {
        Ok(a) => a,
        Err(e) => {
            klog!("elf", "ELF-Lader   : nichts zu laden — {}", e.describe());
            return (0, 0);
        }
    };
    // Der aktive Adressraum. Der unprivilegierte Bereich darin ist leer; die
    // Seiten, die hier entstehen, tragen das Unprivilegiert-Recht und sind
    // damit genau die, in die die naechste Schicht springt.
    let mut space = unsafe { AddressSpace::current() };
    let mut ok = 0usize;
    let mut total = 0usize;

    // 0) Laden und wieder Abraeumen. Zuerst, weil der unprivilegierte Bereich
    //    danach wieder leer sein muss, bevor das echte Programm einzieht:
    //    ein Abbild wird vollstaendig abgebildet, sofort wieder entfernt, und
    //    gemessen wird an den freien Frames, dass wirklich JEDE Seite
    //    zurueckkommt — Segmente wie Stapel. Ohne diesen Weg haette jedes
    //    beendete Programm Speicher liegen lassen.
    total += 1;
    let mut probe = [0u8; 256];
    let plen = build_probe(&mut probe);
    probe[24..32].copy_from_slice(&0x0080_0000u64.to_le_bytes()); // Einsprung
    probe[EHDR_LEN + 16..EHDR_LEN + 24].copy_from_slice(&0x0080_0000u64.to_le_bytes());
    probe[EHDR_LEN + 40..EHDR_LEN + 48]
        .copy_from_slice(&(2 * PAGE_SIZE as u64).to_le_bytes()); // 2 Seiten
    let before = crate::mm::frame::free_frames();
    let round = match unsafe { load(&mut space, &probe[..plen]) } {
        Ok(l) => {
            let mapped = l.pages;
            match unsafe { unload(&mut space, &probe[..plen], Unload::WithStack) } {
                Ok(freed) => {
                    let after = crate::mm::frame::free_frames();
                    // Alles Angelegte muss zurueck sein; was liegen bleibt,
                    // sind hoechstens die Zwischentabellen des Abstiegs (bis
                    // zu drei Ebenen je beruehrter Region — Programm und
                    // Stapel liegen weit auseinander, also zwei Regionen).
                    // Die gehoeren dem Adressraum, nicht dem Abbild.
                    let einbehalten = before.saturating_sub(after);
                    let entry_gone = space.translate(l.entry).is_none();
                    (freed == mapped && einbehalten <= 6 && entry_gone, mapped, freed, einbehalten)
                }
                Err(_) => (false, mapped, 0, 0),
            }
        }
        Err(_) => (false, 0, 0, 0),
    };
    if round.0 {
        ok += 1;
    }
    klog!(
        "elf",
        "  {:<28} -> {} Seiten abgebildet, {} zurueckgegeben, {} Zwischentabelle(n) bleiben, Einsprung nicht mehr abgebildet {}",
        "Abbild geladen und abgeraeumt",
        round.1,
        round.2,
        round.3,
        if round.0 { "(ok)" } else { "(FEHLER)" }
    );

    // 1) Das echte Programm laden und nachpruefen.
    if let Some(bytes) = archive.find("hello") {
        total += 1;
        match unsafe { load_from_initramfs(&mut space, "hello") } {
            Ok(l) => {
                if verify_loaded(&space, bytes, &l) {
                    ok += 1;
                    klog!(
                        "elf",
                        "  nachgeprueft: Einsprungseite und Stapelseite abgebildet, Code stimmt mit der Datei ueberein"
                    );
                } else {
                    klog!("elf", "  FEHLER: geladenes Abbild stimmt nicht mit der Datei ueberein");
                }
            }
            Err(e) => klog!("elf", "  FEHLER: hello nicht ladbar — {}", e.describe()),
        }
    }

    // 2) Ein Abbild aus dem Archiv, dessen Segment in den Kernelbereich zeigt.
    //    Das ist der Negativtest am echten Weg, nicht an einem im Kernel
    //    zusammengebauten Puffer.
    if let Some(bytes) = archive.find("kaputt.elf") {
        total += 1;
        let got = unsafe { load(&mut space, bytes) };
        let good = matches!(got, Err(LoadError::Format(ElfError::SegmentOutOfRange)));
        if good {
            ok += 1;
        }
        klog!(
            "elf",
            "  {:<28} -> {} {}",
            "kaputt.elf aus dem Archiv",
            match &got {
                Ok(_) => "angenommen",
                Err(e) => e.describe(),
            },
            if good { "(ok)" } else { "(FEHLER)" }
        );
    }

    // 3) Eine Textdatei ist kein Programm.
    if let Some(bytes) = archive.find("liesmich.txt") {
        total += 1;
        let got = unsafe { load(&mut space, bytes) };
        let good = matches!(got, Err(LoadError::Format(ElfError::BadMagic)));
        if good {
            ok += 1;
        }
        klog!(
            "elf",
            "  {:<28} -> {} {}",
            "liesmich.txt als Programm",
            match &got {
                Ok(_) => "angenommen",
                Err(e) => e.describe(),
            },
            if good { "(ok)" } else { "(FEHLER)" }
        );
    }

    // 4) Ein Abbild, das zu viele Seiten verlangt: der Lader muss mittendrin
    //    abbrechen UND alles Angelegte wieder zurueckgeben. Gemessen wird an
    //    der Zahl freier Frames vor und nach dem Versuch — ohne Rueckrollen
    //    bliebe hier bei jedem Fehlversuch Speicher liegen.
    total += 1;
    let mut buf = [0u8; 256];
    let len = build_probe(&mut buf);
    buf[24..32].copy_from_slice(&0x0060_0000u64.to_le_bytes()); // Einsprung
    buf[EHDR_LEN + 16..EHDR_LEN + 24].copy_from_slice(&0x0060_0000u64.to_le_bytes()); // vaddr
    buf[EHDR_LEN + 40..EHDR_LEN + 48]
        .copy_from_slice(&((MAX_IMAGE_PAGES as u64 + 8) * PAGE_SIZE as u64).to_le_bytes());
    let before = crate::mm::frame::free_frames();
    let got = unsafe { load(&mut space, &buf[..len]) };
    let after = crate::mm::frame::free_frames();
    // Jede DATENSEITE muss zurueckkommen. Die hoechstens drei Zwischentabellen
    // (eine je Ebene), die der Abstieg fuer diesen Adressbereich angelegt hat,
    // bleiben bewusst stehen: sie gehoeren dem Adressraum, nicht dem Abbild,
    // und werden beim naechsten Mapping in derselben Region wiederverwendet.
    let einbehalten = before.saturating_sub(after);
    let good = matches!(got, Err(LoadError::TooLarge)) && einbehalten <= 3;
    if good {
        ok += 1;
    }
    klog!(
        "elf",
        "  {:<28} -> {}, {} Datenseiten zurueckgegeben, {} Zwischentabelle(n) bleiben {}",
        "Abbild zu gross",
        match &got {
            Ok(_) => "angenommen",
            Err(e) => e.describe(),
        },
        MAX_IMAGE_PAGES,
        einbehalten,
        if good { "(ok)" } else { "(FEHLER)" }
    );

    klog!("elf", "ELF-Ladetest: {}/{} Faelle wie erwartet", ok, total);
    (ok, total)
}
