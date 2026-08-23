//! Anbindung des Bitmap-Rahmenverwalters aus **Firn**.
//!
//! **Diese Datei enthält keine Logik.** Sie erklärt die Symbole aus
//! `kernel/firn/bitmap.fi` und legt eine Hülle darum, die sich genau so
//! benutzen lässt wie die frühere Rust-Fassung in `libs/osum-mem/src/bitmap.rs`
//! — damit `mm/frame.rs` unverändert bleiben konnte.
//!
//! # Warum dieses Modul das zweite in Firn ist
//!
//! Es hält **keinen globalen Zustand**: sowohl der Bitspeicher als auch der
//! Verwaltungsblock gehören dem Aufrufer. Firn hat bis heute kein `static`;
//! ein Modul, das seinen Zustand selbst hielte, ließe sich nicht bauen. Hier
//! stört das nicht, weil der Entwurf ohnehin so herum richtig ist — die
//! Rust-Fassung machte es mit `&'a mut [u64]` genauso.
//!
//! # Was dabei gewonnen ist
//!
//! Die Rechnungen sind **geprüft**. Rust lässt `used -= 1` im Release-Bau
//! still umlaufen; ab da meldet der Verwalter Milliarden freier Rahmen und
//! vergibt Speicher, den es nicht gibt. Die Firn-Fassung bricht an derselben
//! Stelle mit Datei, Zeile, Spalte und beiden Zahlen ab
//! ([`crate::kcore::firn_panic`]).
//!
//! # Aufrufkonvention
//!
//! Die Firn-Seite ist mit `#[export_c]` markiert: die Namen stehen
//! **unverändert** im Objekt, ohne das `_F0.`-Präfix der übrigen
//! Firn-Symbole. Deshalb genügt hier ein gewöhnliches `extern "C"` ohne
//! `#[link_name]`.
//!
//! Der Verwaltungsblock wird als Zeiger übergeben; sein Aufbau
//! ([`FirnBitmap`]) muss Feld für Feld mit `struct Bitmap` in `bitmap.fi`
//! übereinstimmen. `test.sh` prüft das (Abschnitt „Firn-Bitmap").

use core::marker::PhantomData;

/// Rückgabewert von `bm_alloc`, wenn nichts frei ist. Ein gültiger
/// Rahmenindex kann diesen Wert nicht annehmen — er wäre größer als jeder
/// physische Speicher, den es gibt.
const KEIN_RAHMEN: u64 = u64::MAX;

/// Rückgabewerte von `bm_free`.
const E_OK: u64 = 0;

/// Der Verwaltungsblock — Feld für Feld wie `struct Bitmap` in
/// `kernel/firn/bitmap.fi`. Firn legt Strukturen ohne Umsortierung ab, deshalb
/// `#[repr(C)]` auf dieser Seite.
#[repr(C)]
#[derive(Clone, Copy)]
struct FirnBitmap {
    bits: u64,
    frames: u64,
    used: u64,
    cursor: u64,
}

unsafe extern "C" {
    fn bm_words_needed(frames: u64) -> u64;
    fn bm_init(bm: *mut FirnBitmap, bits: u64, woerter: u64, frames: u64) -> bool;
    fn bm_frames(bm: *mut FirnBitmap) -> u64;
    fn bm_used_frames(bm: *mut FirnBitmap) -> u64;
    fn bm_free_frames(bm: *mut FirnBitmap) -> u64;
    fn bm_is_used(bm: *mut FirnBitmap, i: u64) -> bool;
    fn bm_free_range(bm: *mut FirnBitmap, start: u64, ende: u64);
    fn bm_reserve_range(bm: *mut FirnBitmap, start: u64, ende: u64);
    fn bm_alloc(bm: *mut FirnBitmap) -> u64;
    fn bm_alloc_contiguous(bm: *mut FirnBitmap, anzahl: u64) -> u64;
    fn bm_free(bm: *mut FirnBitmap, index: u64, anzahl: u64) -> u64;
}

/// Ein Rahmenindex (physische Adresse / PAGE_SIZE).
pub type FrameIndex = usize;

/// Fehler des Verwalters. Gleichlautend zur früheren Rust-Fassung, damit die
/// Aufrufer unverändert bleiben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    /// Kein passender freier Bereich mehr vorhanden.
    OutOfMemory,
    /// Index liegt ausserhalb der Bitmap.
    OutOfRange,
}

/// Bitmap über `frames` Rahmen. Bit gesetzt = belegt.
///
/// Die Lebenszeit `'a` bindet die Hülle an den geliehenen Bitspeicher — genau
/// wie beim `&'a mut [u64]` der Rust-Fassung. Die Firn-Seite kennt keine
/// Lebenszeiten; diese Zusage wird also **hier** eingehalten, indem der Zeiger
/// nirgends herauskommt.
pub struct Bitmap<'a> {
    inner: FirnBitmap,
    _speicher: PhantomData<&'a mut [u64]>,
}

impl<'a> Bitmap<'a> {
    /// Erzeugt eine Bitmap, in der zunächst **alles** als belegt markiert ist.
    /// Freigegeben wird gezielt über [`Bitmap::free_range`].
    ///
    /// # Panics
    /// Wenn `bits` zu klein für `frames` ist. Die Firn-Seite meldet das mit
    /// `false`; hier wird daraus ein Panic, weil die früheren Aufrufer sich
    /// darauf verlassen (`assert!` in der Rust-Fassung) und ein zu kleiner
    /// Bitspeicher im Boot ohnehin nicht behandelbar ist.
    pub fn new_all_used(bits: &'a mut [u64], frames: usize) -> Self {
        let mut inner = FirnBitmap { bits: 0, frames: 0, used: 0, cursor: 0 };
        // Sicher: `bits` ist ein lebender, exklusiv geliehener Speicher; die
        // Firn-Seite schreibt genau `bits.len()` Wörter hinein und behält den
        // Zeiger nur im Verwaltungsblock, der dieselbe Lebenszeit hat.
        let ok = unsafe {
            bm_init(
                &raw mut inner,
                bits.as_mut_ptr() as u64,
                bits.len() as u64,
                frames as u64,
            )
        };
        assert!(ok, "Bitmap zu klein fuer Framezahl");
        Self { inner, _speicher: PhantomData }
    }

    /// Anzahl der u64-Wörter, die für `frames` Rahmen nötig sind.
    pub fn words_needed(frames: usize) -> usize {
        // Sicher: reine Rechnung, fasst keinen Speicher an.
        (unsafe { bm_words_needed(frames as u64) }) as usize
    }

    /// Gesamtzahl verwalteter Rahmen.
    pub fn frames(&self) -> usize {
        (unsafe { bm_frames(self.zeiger()) }) as usize
    }

    /// Belegte Rahmen.
    pub fn used_frames(&self) -> usize {
        (unsafe { bm_used_frames(self.zeiger()) }) as usize
    }

    /// Freie Rahmen.
    pub fn free_frames(&self) -> usize {
        (unsafe { bm_free_frames(self.zeiger()) }) as usize
    }

    /// Ist der Rahmen belegt? Alles hinter dem Ende gilt als belegt.
    pub fn is_used(&self, i: FrameIndex) -> bool {
        unsafe { bm_is_used(self.zeiger(), i as u64) }
    }

    /// Markiert `[start, end)` als frei (nur Rahmen innerhalb der Bitmap).
    pub fn free_range(&mut self, start: FrameIndex, end: FrameIndex) {
        unsafe { bm_free_range(&raw mut self.inner, start as u64, end as u64) }
    }

    /// Markiert `[start, end)` als belegt.
    pub fn reserve_range(&mut self, start: FrameIndex, end: FrameIndex) {
        unsafe { bm_reserve_range(&raw mut self.inner, start as u64, end as u64) }
    }

    /// Belegt genau einen Rahmen.
    pub fn alloc(&mut self) -> Result<FrameIndex, AllocError> {
        match unsafe { bm_alloc(&raw mut self.inner) } {
            KEIN_RAHMEN => Err(AllocError::OutOfMemory),
            i => Ok(i as FrameIndex),
        }
    }

    /// Belegt `count` zusammenhängende Rahmen (Next-Fit mit Umlauf).
    pub fn alloc_contiguous(&mut self, count: usize) -> Result<FrameIndex, AllocError> {
        match unsafe { bm_alloc_contiguous(&raw mut self.inner, count as u64) } {
            KEIN_RAHMEN => Err(AllocError::OutOfMemory),
            i => Ok(i as FrameIndex),
        }
    }

    /// Gibt `count` Rahmen ab `index` frei.
    pub fn free(&mut self, index: FrameIndex, count: usize) -> Result<(), AllocError> {
        match unsafe { bm_free(&raw mut self.inner, index as u64, count as u64) } {
            E_OK => Ok(()),
            _ => Err(AllocError::OutOfRange),
        }
    }

    /// Zeiger auf den Verwaltungsblock für die lesenden Abfragen.
    ///
    /// Die Firn-Seite nimmt durchgehend `*mut`, auch wo sie nur liest — die
    /// Sprache kennt keinen `*const`. Diese Funktionen ändern nachweislich
    /// nichts (`bitmap.fi`: `bm_frames`, `bm_used_frames`, `bm_free_frames`,
    /// `bm_is_used` bestehen aus einem `return`), deshalb ist der Zeiger aus
    /// einer geteilten Referenz hier zulässig.
    #[inline]
    fn zeiger(&self) -> *mut FirnBitmap {
        &self.inner as *const FirnBitmap as *mut FirnBitmap
    }
}
