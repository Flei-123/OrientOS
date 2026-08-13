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

use crate::kcore::user::{is_user_addr, USER_LIMIT};

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
        if rd32(bytes, p) != PT_LOAD {
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
        if filesz > memsz || memsz == 0 {
            return Err(ElfError::SegmentInsane);
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
    bad[24..32].copy_from_slice(&0u64.to_le_bytes());
    check("Einsprung ausserhalb", Err(ElfError::BadEntry), parse(&bad[..len]));

    crate::klog!("elf", "ELF-Negativtest: {}/{} Faelle wie erwartet abgewiesen", ok, total);
    (ok, total)
}
