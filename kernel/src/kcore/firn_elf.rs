//! Anbindung des ELF64-**Prüfteils** aus Firn.
//!
//! **Diese Datei enthält keine Prüflogik.** Sie erklärt die Symbole aus
//! `kernel/firn/elf.fi` und legt eine Hülle darum, die genauso aussieht wie
//! der frühere Rust-Prüfteil — damit der Ladeteil in
//! [`crate::kcore::elf`] unverändert bleiben konnte.
//!
//! # Warum dieser Teil in Firn liegt
//!
//! Ein ELF kommt von außen. Jede Zahl darin ist die **Behauptung eines
//! Fremden**: „meine Tabelle beginnt bei Offset X", „mein Segment ist Y lang".
//! Wer damit ungeprüft rechnet, baut sich die Lücke, die Angreifer suchen —
//! ein `off + filesz`, das umläuft, ergibt eine kleine Zahl, die
//! Bereichsprüfung geht durch, und danach wird an einer Adresse gelesen, die
//! nie geprüft wurde.
//!
//! Die Rust-Fassung hat das mit `checked_add` von Hand abgefangen, an jeder
//! einzelnen Stelle, und hat gehalten — 53 von 53 Fällen. In Firn ist es die
//! **Sprache**: jede Rechnung, die überläuft, bricht ab, auch die, an die
//! niemand gedacht hat.
//!
//! # Was **nicht** hier liegt
//!
//! Der Ladeteil (Seiten anlegen, Inhalte kopieren, Zurückrollen) bleibt in
//! Rust. Er hängt an Seitentabellen, Rahmenverwalter und Direct-Map-Fenster —
//! ein Dutzend Rückrufe. Er fasst aber auch **keine fremden Bytes** an: er
//! arbeitet mit den Werten, die hier bereits geprüft wurden.
//!
//! # Nachweis
//!
//! `tests/firn-elf/` — 53 Falldateien, gefahren gegen **beide** Fassungen mit
//! demselben Ergebnis. `lauf-rust.sh` schneidet den alten Prüfteil aus der
//! Kernel-Datei und fährt ihn auf dem Host; `lauf.sh` fährt dieselben Dateien
//! gegen das Firn-Objekt.

use core::mem::MaybeUninit;

/// Höchstzahl der Segmente, die der Lader annimmt. Muss mit `MAX_SEGMENTS` in
/// `kernel/firn/elf.fi` übereinstimmen; `elf_max_segments()` gibt den Wert der
/// Firn-Seite zurück, und `test.sh` vergleicht beide.
pub const MAX_SEGMENTS: usize = 16;

/// Ein Segment, wie die Firn-Seite es ablegt. Feld für Feld wie
/// `struct ElfSegment` in `elf.fi`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FirnSegment {
    vaddr: u64,
    file_len: u64,
    mem_len: u64,
    file_off: u64,
    /// ELF-Rechtebits, **unverändert** (`PF_R`=4, `PF_W`=2, `PF_X`=1).
    /// Die Firn-Seite deutet sie bewusst nicht um.
    flags: u64,
}

/// Das Ergebnis, wie die Firn-Seite es ablegt. Feld für Feld wie
/// `struct ElfImage` in `elf.fi`.
#[repr(C)]
struct FirnImage {
    entry: u64,
    count: u64,
    segments: [FirnSegment; MAX_SEGMENTS],
}

unsafe extern "C" {
    fn elf_parse(p: u64, len: u64, img: *mut FirnImage) -> u64;
    fn elf_max_segments() -> u64;
}

/// Was die Firn-Seite an Segmenten kennt — für den Abgleich im Testlauf.
pub fn firn_max_segments() -> usize {
    (unsafe { elf_max_segments() }) as usize
}

/// Warum ein Abbild nicht geladen werden kann.
///
/// Die Zahlenwerte sind Teil der Schnittstelle: sie stehen genauso in
/// `kernel/firn/elf.fi`, in `tests/firn-elf/faelle.py` und im Prüfstand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u64)]
pub enum ElfError {
    /// Datei kuerzer als der Kopf.
    TooShort = 1,
    /// Kennung am Dateianfang stimmt nicht.
    BadMagic = 2,
    /// Keine 64-Bit-Datei.
    BadClass = 3,
    /// Falsche Bytereihenfolge.
    BadEndian = 4,
    /// Kein ausfuehrbares Abbild (z. B. Objektdatei oder dynamisch gebunden).
    BadType = 5,
    /// Falsche Zielarchitektur.
    BadMachine = 6,
    /// Tabelle der Programmkoepfe liegt ausserhalb der Datei.
    BadTable = 7,
    /// Ein Segment liegt ausserhalb der Datei.
    SegmentOutOfFile = 8,
    /// Ein Segment liegt ausserhalb des unprivilegierten Adressbereichs.
    SegmentOutOfRange = 9,
    /// Zwei Segmente ueberlappen sich.
    SegmentOverlap = 10,
    /// Zwei Segmente teilen sich eine Seite. Sie haetten dann zwangslaeufig
    /// dieselben Rechte — genau das, was die Trennung in Segmente verhindern
    /// soll.
    SegmentsSharePage = 11,
    /// Segmentangaben sind in sich unstimmig (z. B. Dateilaenge > Speicherlaenge).
    SegmentInsane = 12,
    /// Ausrichtungsangabe des Segments ist keine Zweierpotenz oder Datei- und
    /// Speicherlage passen modulo dieser Ausrichtung nicht zueinander.
    BadAlign = 13,
    /// Das Abbild verlangt einen Binder (`PT_INTERP`) — der Kern bindet nicht.
    NeedsInterpreter = 14,
    /// Einsprungadresse liegt in keinem geladenen Segment.
    BadEntry = 15,
    /// Mehr Segmente, als der Lader annimmt.
    TooManySegments = 16,
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
            ElfError::SegmentsSharePage => "zwei Segmente laegen in derselben Seite",
            ElfError::SegmentInsane => "unstimmige Segmentangaben",
            ElfError::BadAlign => "unstimmige Ausrichtungsangabe",
            ElfError::NeedsInterpreter => "dynamisch gebunden, verlangt einen Binder",
            ElfError::BadEntry => "Einsprungadresse in keinem Segment",
            ElfError::TooManySegments => "zu viele Segmente",
        }
    }

    /// Wandelt den Code der Firn-Seite in den Fehlerwert.
    ///
    /// Ein **unbekannter** Code wird zu `TooShort` — bewusst zu einem Fehler
    /// und niemals zu „angenommen". Käme aus einer neueren Firn-Fassung eine
    /// Zahl, die es hier noch nicht gibt, wäre „Abbild in Ordnung" die
    /// gefährlichste aller Antworten.
    fn from_code(c: u64) -> Self {
        match c {
            1 => ElfError::TooShort,
            2 => ElfError::BadMagic,
            3 => ElfError::BadClass,
            4 => ElfError::BadEndian,
            5 => ElfError::BadType,
            6 => ElfError::BadMachine,
            7 => ElfError::BadTable,
            8 => ElfError::SegmentOutOfFile,
            9 => ElfError::SegmentOutOfRange,
            10 => ElfError::SegmentOverlap,
            11 => ElfError::SegmentsSharePage,
            12 => ElfError::SegmentInsane,
            13 => ElfError::BadAlign,
            14 => ElfError::NeedsInterpreter,
            15 => ElfError::BadEntry,
            16 => ElfError::TooManySegments,
            _ => ElfError::TooShort,
        }
    }
}

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

/// Prueft ein Abbild vollstaendig durch und liefert die Ladeauftraege.
///
/// Fuehrt KEINE Speicherzugriffe ausserhalb von `bytes` aus und bildet nichts
/// ab. Die Prüfung selbst macht `kernel/firn/elf.fi`.
pub fn parse(bytes: &[u8]) -> Result<Image, ElfError> {
    // Nicht genullt vorbereiten: bei Erfolg beschreibt die Firn-Seite `entry`,
    // `count` und genau `count` Segmente. Was sie nicht beschreibt, wird unten
    // auch nicht gelesen.
    let mut roh: MaybeUninit<FirnImage> = MaybeUninit::uninit();

    // Sicher: `bytes` ist ein lebender Speicherbereich; die Firn-Seite liest
    // ausschliesslich innerhalb von [ptr, ptr+len) und schreibt ausschliesslich
    // in `roh`. Ein leerer Schnitt liefert einen ausgerichteten, aber nicht
    // benutzbaren Zeiger — die Firn-Seite fasst ihn bei `len == 0` nicht an,
    // weil sie zuerst gegen die Kopflaenge prueft.
    let code = unsafe { elf_parse(bytes.as_ptr() as u64, bytes.len() as u64, roh.as_mut_ptr()) };
    if code != 0 {
        return Err(ElfError::from_code(code));
    }

    // Erfolg: jetzt ist `roh` beschrieben.
    let roh = unsafe { roh.assume_init() };

    // `count` kommt aus der Firn-Seite. Sie kann laut ihrer eigenen Prüfung
    // nicht über MAX_SEGMENTS hinausgehen — hier wird es trotzdem gekappt,
    // weil dieser Wert sonst einen Zugriff jenseits des Feldes steuern würde.
    // Eine Grenze, die von der Richtigkeit eines anderen Moduls abhängt, ist
    // keine Grenze.
    let count = (roh.count as usize).min(MAX_SEGMENTS);

    let mut img = Image { entry: roh.entry, segments: [Segment::default(); MAX_SEGMENTS], count };
    for i in 0..count {
        let s = roh.segments[i];
        img.segments[i] = Segment {
            vaddr: s.vaddr,
            file_len: s.file_len,
            mem_len: s.mem_len,
            file_off: s.file_off,
            read: s.flags & 4 != 0,
            write: s.flags & 2 != 0,
            exec: s.flags & 1 != 0,
        };
    }
    Ok(img)
}
