//! Startdateisystem (Initramfs).
//!
//! **Schicht: core.** Der Kernel bekommt vom Bootloader einen zusammen-
//! haengenden Speicherbereich mit einem Archiv darin. Diese Datei kennt das
//! Archivformat und liefert die Dateien als Byte-Scheiben; sie laedt nichts
//! und bildet nichts ab.
//!
//! ## Warum ein eigenes Format und nicht cpio oder tar
//!
//! * cpio und tar transportieren POSIX-Metadaten (Modus, uid/gid, mtime,
//!   Verzeichnisse, Symlinks, Pfade mit `/`). Der Kern dieses Systems kennt
//!   davon nichts: es gibt keine ambient authority, keinen globalen
//!   Pfadnamensraum und keine Benutzerkennungen. Ein cpio-Leser muesste diese
//!   Felder lesen und sofort wegwerfen — Code, der nur Angriffsflaeche ist.
//! * Beide Formate sind stromorientiert: um den n-ten Eintrag zu finden, muss
//!   man n-1 Koepfe entlanghangeln und dabei ASCII-Oktalzahlen auswerten.
//! * Das Format hier ist eine Tabelle fester Breite am Anfang. Anzahl und
//!   Gesamtlaenge stehen im Kopf, jeder Eintrag hat Offset und Laenge als
//!   `u64`. Alle Grenzen werden mit `checked`-Arithmetik gegen die
//!   tatsaechliche Archivlaenge geprueft, bevor irgendetwas gelesen wird.
//!
//! ## Aufbau (alles little-endian)
//!
//! ```text
//! 0x00  8 B   Kennung "IRFS0001"
//! 0x08  u32   Anzahl Eintraege
//! 0x0c  u32   Gesamtlaenge des Archivs in Bytes
//! 0x10  Tabelle, Anzahl * 48 B:
//!         +0x00  32 B  Name, mit Nullbytes aufgefuellt (kein '/', ASCII)
//!         +0x20  u64   Offset der Daten ab Archivanfang
//!         +0x28  u64   Laenge der Daten
//! danach: die Daten, jeweils auf 16 B ausgerichtet.
//! ```
//!
//! Gebaut wird das Archiv von `userland/mkinitramfs.py`, eingebunden ueber
//! `limine.conf` (`module_string: initramfs`).

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::kcore::mem::VirtAddr;
use crate::klog;

/// Kennung am Archivanfang.
const MAGIC: &[u8; 8] = b"IRFS0001";
/// Laenge des Archivkopfs.
const HEADER_LEN: usize = 16;
/// Laenge eines Tabelleneintrags.
const ENTRY_LEN: usize = 48;
/// Laenge des Namensfelds eines Eintrags.
const NAME_LEN: usize = 32;
/// Obergrenze fuer die Eintragszahl — schuetzt vor einem Kopf, der Millionen
/// Eintraege behauptet.
const MAX_ENTRIES: usize = 1024;

static BASE: AtomicU64 = AtomicU64::new(0);
static LEN: AtomicUsize = AtomicUsize::new(0);
static PULLED: AtomicBool = AtomicBool::new(false);

/// Uebernimmt den vom Bootloader gelieferten Archivbereich.
///
/// # Safety
/// `base`/`len` muessen einen gueltigen, bis zum Ende der Laufzeit gueltigen
/// Bereich beschreiben (Module des Bootloaders liegen dauerhaft im Speicher).
pub unsafe fn set(base: VirtAddr, len: usize) {
    BASE.store(base.as_u64(), Ordering::SeqCst);
    LEN.store(len, Ordering::SeqCst);
    PULLED.store(true, Ordering::SeqCst);
}

/// Holt den Bereich beim ersten Zugriff aus der Bootschicht.
///
/// Der Kern fragt hier aktiv nach, statt darauf zu warten, dass ihn jemand
/// fuettert: so ist das Archiv ab dem ersten Zugriff verfuegbar, egal in
/// welcher Reihenfolge der Startvorgang die Schichten anfasst.
fn ensure() {
    if PULLED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some((addr, len)) = crate::boot::limine::module_by_string("initramfs") {
        BASE.store(addr, Ordering::SeqCst);
        LEN.store(len, Ordering::SeqCst);
    }
}

/// Der Archivbereich als Byte-Scheibe, falls einer uebergeben wurde.
pub fn image() -> Option<&'static [u8]> {
    ensure();
    let b = BASE.load(Ordering::SeqCst);
    let l = LEN.load(Ordering::SeqCst);
    if b == 0 || l == 0 {
        return None;
    }
    // SAFETY: nur lesend; der Bereich stammt vom Bootloader bzw. aus [`set`]
    // und bleibt fuer die gesamte Laufzeit gueltig (Regionstyp
    // "kernel+modules", der Frame-Allocator vergibt ihn nie).
    Some(unsafe { core::slice::from_raw_parts(b as *const u8, l) })
}

/// Grund, warum ein Archiv nicht benutzbar ist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArchiveError {
    /// Kein Archiv uebergeben.
    Missing,
    /// Kuerzer als der Kopf.
    TooShort,
    /// Kennung stimmt nicht.
    BadMagic,
    /// Kopf behauptet eine andere Laenge als der Bereich hat.
    BadLength,
    /// Mehr Eintraege als erlaubt oder als in den Bereich passen.
    BadCount,
    /// Ein Eintrag zeigt aus dem Archiv heraus.
    EntryOutOfRange,
    /// Ein Name ist leer oder nicht abschliessend genullt.
    BadName,
    /// Zwei Eintraege tragen denselben Namen.
    DuplicateName,
}

impl ArchiveError {
    /// Klartext fuer das Boot-Log.
    pub const fn describe(self) -> &'static str {
        match self {
            ArchiveError::Missing => "kein Archiv uebergeben",
            ArchiveError::TooShort => "kuerzer als der Archivkopf",
            ArchiveError::BadMagic => "falsche Kennung am Archivanfang",
            ArchiveError::BadLength => "Laengenangabe passt nicht zum Bereich",
            ArchiveError::BadCount => "unplausible Eintragszahl",
            ArchiveError::EntryOutOfRange => "Eintrag zeigt aus dem Archiv heraus",
            ArchiveError::BadName => "unbrauchbarer Name",
            ArchiveError::DuplicateName => "zwei Eintraege mit demselben Namen",
        }
    }
}

/// Ein geprueftes Archiv.
#[derive(Clone, Copy)]
pub struct Archive<'a> {
    bytes: &'a [u8],
    count: usize,
}

fn rd32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn rd64(b: &[u8], off: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[off..off + 8]);
    u64::from_le_bytes(v)
}

/// Prueft einen Bereich vollstaendig durch, bevor irgendein Eintrag gelesen
/// wird. Fuehrt keinen Zugriff ausserhalb von `bytes` aus.
pub fn open(bytes: &[u8]) -> Result<Archive<'_>, ArchiveError> {
    if bytes.len() < HEADER_LEN {
        return Err(ArchiveError::TooShort);
    }
    if &bytes[..8] != MAGIC {
        return Err(ArchiveError::BadMagic);
    }
    let count = rd32(bytes, 8) as usize;
    let total = rd32(bytes, 12) as usize;
    if total != bytes.len() {
        return Err(ArchiveError::BadLength);
    }
    if count > MAX_ENTRIES {
        return Err(ArchiveError::BadCount);
    }
    let table = count.checked_mul(ENTRY_LEN).ok_or(ArchiveError::BadCount)?;
    let data_start = HEADER_LEN.checked_add(table).ok_or(ArchiveError::BadCount)?;
    if data_start > bytes.len() {
        return Err(ArchiveError::BadCount);
    }
    for i in 0..count {
        let base = HEADER_LEN + i * ENTRY_LEN;
        let name = &bytes[base..base + NAME_LEN];
        if name[0] == 0 || name[NAME_LEN - 1] != 0 || name.contains(&b'/') {
            return Err(ArchiveError::BadName);
        }
        let off = usize::try_from(rd64(bytes, base + NAME_LEN))
            .map_err(|_| ArchiveError::EntryOutOfRange)?;
        let len = usize::try_from(rd64(bytes, base + NAME_LEN + 8))
            .map_err(|_| ArchiveError::EntryOutOfRange)?;
        let end = off.checked_add(len).ok_or(ArchiveError::EntryOutOfRange)?;
        if off < data_start || end > bytes.len() {
            return Err(ArchiveError::EntryOutOfRange);
        }
        // Doppelte Namen sind kein Schoenheitsfehler: `find` wuerde
        // stillschweigend einen der beiden Eintraege waehlen, und welcher das
        // ist, entschiede der Packer. Wer ein Programm laedt, soll genau
        // wissen, welche Bytes er bekommt — also wird das Archiv abgewiesen.
        for j in 0..i {
            let other = HEADER_LEN + j * ENTRY_LEN;
            if bytes[other..other + NAME_LEN] == *name {
                return Err(ArchiveError::DuplicateName);
            }
        }
    }
    Ok(Archive { bytes, count })
}

impl<'a> Archive<'a> {
    /// Anzahl Eintraege.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Ist das Archiv leer?
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Gesamtlaenge in Bytes.
    pub fn bytes(&self) -> usize {
        self.bytes.len()
    }

    /// Name und Inhalt des `i`-ten Eintrags.
    pub fn entry(&self, i: usize) -> Option<(&'a str, &'a [u8])> {
        if i >= self.count {
            return None;
        }
        let base = HEADER_LEN + i * ENTRY_LEN;
        let raw = &self.bytes[base..base + NAME_LEN];
        let nlen = raw.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
        let name = core::str::from_utf8(&raw[..nlen]).ok()?;
        let off = rd64(self.bytes, base + NAME_LEN) as usize;
        let len = rd64(self.bytes, base + NAME_LEN + 8) as usize;
        Some((name, &self.bytes[off..off + len]))
    }

    /// Sucht einen Eintrag nach Namen.
    pub fn find(&self, name: &str) -> Option<&'a [u8]> {
        (0..self.count)
            .filter_map(|i| self.entry(i))
            .find(|(n, _)| *n == name)
            .map(|(_, d)| d)
    }
}

/// Das geprueft geoeffnete Archiv, falls eines uebergeben wurde und es in
/// Ordnung ist.
pub fn archive() -> Result<Archive<'static>, ArchiveError> {
    let bytes = image().ok_or(ArchiveError::Missing)?;
    open(bytes)
}

/// Sucht eine Datei im Archiv.
pub fn find(name: &str) -> Option<&'static [u8]> {
    archive().ok().and_then(|a| a.find(name))
}

/// Anzahl der Eintraege im Archiv (0, wenn keines oder ein kaputtes vorliegt).
pub fn count() -> usize {
    archive().map(|a| a.len()).unwrap_or(0)
}

/// Boot-Meldung: was der Bootloader geliefert hat.
pub fn report() {
    match archive() {
        Ok(a) => {
            klog!("init", "Initramfs   : {} Eintraege, {} B", a.len(), a.bytes());
            for i in 0..a.len() {
                if let Some((name, data)) = a.entry(i) {
                    klog!("init", "  Eintrag {}: {:<14} {} B", i, name, data.len());
                }
            }
        }
        Err(ArchiveError::Missing) => {
            klog!("init", "Initramfs   : kein Archiv uebergeben")
        }
        Err(e) => klog!("init", "Initramfs   : unbrauchbar — {}", e.describe()),
    }
}

/// Negativtest des Archivlesers: jede Sorte kaputter Kopf muss mit dem
/// PASSENDEN Fehler abgewiesen werden, und keiner darf den Kernel anhalten.
///
/// Der Test arbeitet auf einem im Kernel gebauten Puffer — er braucht also
/// kein Archiv vom Bootloader und laeuft in jedem Boot.
///
/// Liefert `(bestanden, gesamt)`.
pub fn selftest() -> (usize, usize) {
    // Ein winziges, gueltiges Archiv mit einem Eintrag "a" (4 B Daten).
    const TOTAL: usize = HEADER_LEN + ENTRY_LEN + 4;
    let mut good = [0u8; TOTAL];
    good[..8].copy_from_slice(MAGIC);
    good[8..12].copy_from_slice(&1u32.to_le_bytes());
    good[12..16].copy_from_slice(&(TOTAL as u32).to_le_bytes());
    good[HEADER_LEN] = b'a';
    good[HEADER_LEN + NAME_LEN..HEADER_LEN + NAME_LEN + 8]
        .copy_from_slice(&((HEADER_LEN + ENTRY_LEN) as u64).to_le_bytes());
    good[HEADER_LEN + NAME_LEN + 8..HEADER_LEN + NAME_LEN + 16]
        .copy_from_slice(&4u64.to_le_bytes());

    let mut ok = 0usize;
    let mut total = 0usize;
    let mut check = |name: &str, expect: Result<usize, ArchiveError>, buf: &[u8]| {
        total += 1;
        let got = open(buf);
        let good = match (&expect, &got) {
            (Ok(n), Ok(a)) => a.len() == *n,
            (Err(e), Err(g)) => e == g,
            _ => false,
        };
        if good {
            ok += 1;
        }
        klog!(
            "init",
            "  {:<28} -> {} {}",
            name,
            match &got {
                Ok(a) => {
                    let _ = a;
                    "angenommen"
                }
                Err(e) => e.describe(),
            },
            if good { "(ok)" } else { "(FEHLER)" }
        );
    };

    check("gueltiges Archiv", Ok(1), &good);
    check("leerer Bereich", Err(ArchiveError::TooShort), &[]);

    let mut bad = good;
    bad[0] = b'X';
    check("falsche Kennung", Err(ArchiveError::BadMagic), &bad);

    let mut bad = good;
    bad[12..16].copy_from_slice(&(TOTAL as u32 + 16).to_le_bytes());
    check("falsche Gesamtlaenge", Err(ArchiveError::BadLength), &bad);

    let mut bad = good;
    bad[8..12].copy_from_slice(&99u32.to_le_bytes());
    check("zu viele Eintraege", Err(ArchiveError::BadCount), &bad);

    let mut bad = good;
    bad[HEADER_LEN + NAME_LEN + 8..HEADER_LEN + NAME_LEN + 16]
        .copy_from_slice(&4096u64.to_le_bytes());
    check("Eintrag ausserhalb", Err(ArchiveError::EntryOutOfRange), &bad);

    let mut bad = good;
    bad[HEADER_LEN] = 0;
    check("leerer Name", Err(ArchiveError::BadName), &bad);

    // Zwei Eintraege, ein Name: mehrdeutig, also unbrauchbar.
    const TOTAL2: usize = HEADER_LEN + 2 * ENTRY_LEN + 4;
    let mut zwei = [0u8; TOTAL2];
    zwei[..8].copy_from_slice(MAGIC);
    zwei[8..12].copy_from_slice(&2u32.to_le_bytes());
    zwei[12..16].copy_from_slice(&(TOTAL2 as u32).to_le_bytes());
    for i in 0..2 {
        let base = HEADER_LEN + i * ENTRY_LEN;
        zwei[base] = b'a';
        zwei[base + NAME_LEN..base + NAME_LEN + 8]
            .copy_from_slice(&((HEADER_LEN + 2 * ENTRY_LEN) as u64).to_le_bytes());
        zwei[base + NAME_LEN + 8..base + NAME_LEN + 16].copy_from_slice(&4u64.to_le_bytes());
    }
    check("zweimal derselbe Name", Err(ArchiveError::DuplicateName), &zwei);

    klog!("init", "Archiv-Negativtest: {}/{} Faelle wie erwartet abgewiesen", ok, total);
    (ok, total)
}
