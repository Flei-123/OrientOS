//! `osum_panic` — der Ausgang, den Firns **geprüfte Arithmetik** aufruft.
//!
//! **Schicht: core.** Das ist keine Rust-Logik, die irgendwann nach Firn
//! wandert, sondern eine **Schnittstelle**: Firn erzeugt im `profile kernel`
//! keinen eigenen Panik-Pfad, weil ein Kernel selbst entscheiden muss, was ein
//! Programmfehler bedeutet. Der Übersetzer ruft stattdessen ein **externes**
//! Symbol `osum_panic` — und genau das steht hier.
//!
//! # Woher das kommt
//!
//! Seit Firn-Runde 72/83 sind `+`, `-`, `*`, `/` und Bereichsumwandlungen
//! **geprüft**: läuft eine Rechnung über, bricht sie ab, statt still
//! umzulaufen. Im Anwendungsprofil schreibt Firns Laufzeit die Meldung selbst
//! nach `stderr` und beendet den Prozess. Im Kernelprofil gibt es weder das
//! eine noch das andere — dort endet der Prüfpfad in einem Sprung nach
//! `osum_panic`, gefolgt von `ud2` für den Fall, dass jemand zurückkehrt.
//!
//! Das ist der Grund, warum `kernel/firn/serial.fi` seit dem Übersetzerwechsel
//! auf `4536a191` **ein** undefiniertes Symbol hat statt keines: die
//! Schleifenzähler in `serial_write_bytes` werden jetzt geprüft. `build.sh`
//! lässt deshalb genau diesen einen Namen zu und sonst keinen.
//!
//! # Aufrufkonvention (von Firn vorgegeben, SysV-AMD64)
//!
//! | Register | Inhalt |
//! |---|---|
//! | `rdi` | Zeiger auf den Meldungstext (UTF-8, **ohne** NUL) |
//! | `esi` | Länge des Textes in Oktetten |
//! | `rdx` | erster Operand `a` |
//! | `rcx` | zweiter Operand `b` |
//! | `r8`  | Art des Fehlers, siehe [`Art`] |
//!
//! Die Funktion kehrt **nicht** zurück.
//!
//! # Warum das ein Gewinn ist
//!
//! Rust prüft Überläufe im `release`-Profil **nicht** — `a + b` läuft dort
//! still um. Genau dieses Verhalten hat osum heute an jeder Stelle, die nicht
//! von Hand `checked_*` benutzt. Jede Zeile, die nach Firn wandert, bekommt die
//! Prüfung geschenkt, und ein Überlauf hört auf, ein stiller Datenfehler zu
//! sein: er wird zu einer Meldung mit Datei, Zeile, Spalte und **beiden
//! Zahlen**.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Zählt, wie oft die geprüfte Arithmetik angeschlagen hat. Im Normalbetrieb
/// muss das 0 bleiben; der Selbsttest liest den Wert.
static ANSCHLAEGE: AtomicUsize = AtomicUsize::new(0);

/// Rekursionssperre. Sie ist nicht theoretisch: die serielle Konsole ist
/// selbst in Firn geschrieben, ihre Arithmetik ist also ebenfalls geprüft.
/// Schlägt eine Prüfung **in** `serial.fi` an, will `osum_panic` die Meldung
/// über genau die Konsole ausgeben, die gerade gescheitert ist — und ruft sich
/// dabei wieder selbst.
///
/// Genau das ist beim Übersetzerwechsel auf `4536a191` passiert: `in al, dx`
/// beschreibt nur `al`, die oberen Bits von `rax` blieben stehen, die nun
/// geprüfte Umwandlung `as u8` schlug an — und der Kernel hing **vor** der
/// ersten Zeile Ausgabe. Ohne diese Sperre sieht man in so einem Fall nichts
/// und sucht im falschen Modul.
static IM_PANIK: AtomicBool = AtomicBool::new(false);

/// Wie oft die geprüfte Arithmetik seit dem Start angeschlagen hat.
pub fn anschlaege() -> usize {
    ANSCHLAEGE.load(Ordering::Relaxed)
}

/// Fehlerarten, die Firn in `r8` übergibt. Die Zahlen sind Teil der
/// Schnittstelle und stehen so in `compiler/src/panic_rt.rs` des Firn-Repos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Art {
    /// Überlauf bei `+`.
    Addition = 1,
    /// Überlauf bei `-` (auch: Unterlauf bei vorzeichenlosen Zahlen).
    Subtraktion = 2,
    /// Überlauf bei `*`.
    Multiplikation = 3,
    /// Division durch null.
    DivisionDurchNull = 4,
    /// `i64::MIN / -1` — das Ergebnis passt nicht in den Typ.
    DivisionUeberlauf = 5,
    /// Bereichsumwandlung, deren Wert nicht in den Zieltyp passt.
    Umwandlung = 6,
}

impl Art {
    /// Klartext für die Ausgabe. Unbekannte Codes bleiben unbekannt, statt
    /// geraten zu werden — ein neuer Firn-Code soll auffallen.
    fn beschreibung(code: u64) -> &'static str {
        match code {
            1 => "Ueberlauf bei einer Addition",
            2 => "Ueberlauf bei einer Subtraktion",
            3 => "Ueberlauf bei einer Multiplikation",
            4 => "Division durch null",
            5 => "Division mit nicht darstellbarem Ergebnis",
            6 => "Bereichsumwandlung ausserhalb des Zieltyps",
            _ => "unbekannte Pruefung (neuer Firn-Code?)",
        }
    }
}

/// Der Einsprung, den Firns Prüfpfad aufruft.
///
/// # Safety
/// Wird ausschliesslich von übersetztem Firn-Code aufgerufen. `msg`/`len`
/// zeigen in den `.rodata`-Abschnitt desselben Objekts und leben so lange wie
/// der Kernel.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osum_panic(msg: *const u8, len: u32, a: u64, b: u64, art: u64) -> ! {
    ANSCHLAEGE.fetch_add(1, Ordering::Relaxed);

    // Zweiter Eintritt: die Ausgabe selbst hat die Prüfung ausgelöst. Reden
    // hilft dann nicht mehr — anhalten ist die einzige ehrliche Reaktion.
    if IM_PANIK.swap(true, Ordering::SeqCst) {
        crate::arch::halt_forever();
    }

    // Die Meldung kommt aus dem Objekt selbst und ist gültiges UTF-8; sie
    // enthält Datei, Zeile und Spalte der Rechnung. Ein defekter Zeiger wäre
    // ein Übersetzerfehler — dann ist ein Absturz hier das ehrliche Ergebnis.
    let text = unsafe { core::slice::from_raw_parts(msg, len as usize) };
    let text = core::str::from_utf8(text).unwrap_or("<Meldung nicht lesbar>");

    // Kein `panic!()`: der Rust-Panic-Handler formatiert über `core::fmt` und
    // läuft dabei durch mehr Code, als in dieser Lage nötig ist. Die Zahlen
    // werden direkt ausgegeben — sie sind der eigentliche Befund.
    crate::serial_println!("\n=========== FIRN: GEPRUEFTE RECHNUNG ===========");
    crate::serial_println!("Art     : {}", Art::beschreibung(art));
    crate::serial_println!("Ort     : {}", text);
    crate::serial_println!("Werte   : a={} (0x{:x})  b={} (0x{:x})", a, a, b, b);
    crate::serial_println!("===============================================");

    // Von hier aus in den gewöhnlichen Panic-Pfad: der bringt Backtrace,
    // Laufzeit und das Anhalten mit. Ein Überlauf ist ein Programmfehler und
    // wird genauso behandelt wie jeder andere.
    panic!("geprüfte Rechnung fehlgeschlagen: {}", Art::beschreibung(art))
}
