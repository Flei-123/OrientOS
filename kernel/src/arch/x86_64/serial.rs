//! 16550-UART auf COM1 — die Debugkonsole des fruehen Boots.
//!
//! **Diese Datei enthaelt keine Logik mehr.** Der UART wird seit dem
//! Brueckenkopf von `kernel/firn/serial.fi` bedient — in **Firn**, uebersetzt
//! zu einem freistehenden ELF-Objekt und dazugelinkt. Hier steht nur noch die
//! Erklaerung der Symbole und die Umrechnung von `&str` auf Adresse+Laenge.
//!
//! Warum ausgerechnet dieses Modul zuerst: es ist ein **Blatt**. Es ruft nichts
//! ausserhalb seiner selbst auf und haelt keinen Zustand. Stage 0 von Firn hat
//! weder `extern fn` noch `static` — ein Modul, das eines von beidem braucht,
//! liesse sich heute nicht bauen. Begruendung und Migrationsstand:
//! [`LANGUAGE.md`](../../../../LANGUAGE.md).
//!
//! Bewusst **ohne Sperre**: Der Panic-Handler muss auch dann noch schreiben
//! koennen, wenn ein Interrupt mitten in einer Ausgabe zugeschlagen hat. Ein
//! Spinlock wuerde in genau dieser Lage verklemmen. Solange nur eine CPU laeuft,
//! ist das gefahrlos; mit SMP kommt ein Per-CPU-Puffer (siehe ROADMAP.md).
//!
//! Aufrufkonvention: Firn erzeugt gewoehnliches SysV-AMD64 — Argumente in
//! `rdi`/`rsi`, Rueckgabe in `rax`, `rbp` und `r12`–`r15` werden gerettet.
//! Damit ist `extern "C"` die richtige und einzige noetige Deklaration.
//! Das Symbolschema von `firnc` ist `_F0.<name>`; der Punkt ist in Rust kein
//! gueltiger Bezeichner, deshalb `#[link_name = …]`.

unsafe extern "C" {
    /// COM1 auf 115200 8N1 mit aktivem FIFO. `kernel/firn/serial.fi`.
    #[link_name = "_F0.serial_init"]
    fn firn_serial_init();

    /// Einzelnes Byte, blockierend mit Notbremse. `kernel/firn/serial.fi`.
    #[link_name = "_F0.serial_write_byte"]
    fn firn_serial_write_byte(b: u8);

    /// `n` Oktette ab `p`; `\n` wird zu `\r\n`. `kernel/firn/serial.fi`.
    #[link_name = "_F0.serial_write_bytes"]
    fn firn_serial_write_bytes(p: u64, n: u64);
}

/// Initialisiert COM1 auf 115200 8N1 mit aktivem FIFO.
///
/// # Safety
/// Programmiert Hardware; genau einmal im Boot.
pub unsafe fn init() {
    unsafe { firn_serial_init() }
}

/// Einzelnes Byte senden (blockierend, mit Notbremse).
#[inline]
pub fn write_byte(b: u8) {
    // Sicher: die Firn-Seite fasst nur den IO-Port an und liest keinen Speicher.
    unsafe { firn_serial_write_byte(b) }
}

/// Zeichenkette senden. `\n` wird zu `\r\n`, damit Terminals sauber umbrechen.
pub fn write_str(s: &str) {
    // Sicher: Adresse und Laenge stammen aus demselben lebenden `&str`, und die
    // Firn-Seite liest ausschliesslich daraus — sie schreibt nichts zurueck.
    unsafe { firn_serial_write_bytes(s.as_ptr() as u64, s.len() as u64) }
}
