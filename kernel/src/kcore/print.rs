//! Textausgabe der core-Schicht: `print!`, `println!`, `serial_print!`,
//! `serial_println!`.
//!
//! **Schicht: core.** Kennt weder UART-Ports noch Framebuffer-Formate.
//! Der serielle Kanal wird ueber [`crate::arch`] bedient (arch-Trait), zusaetzliche
//! Senken (z. B. der Framebuffer-Textkonsole aus `drivers`) registrieren sich
//! zur Laufzeit ueber [`set_sink`].

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicPtr, Ordering};

/// Eine Textsenke — implementiert von Konsolentreibern in der `drivers`-Schicht.
pub trait TextSink: Sync + Send {
    /// Gibt einen UTF-8-String aus. Muss selbst fuer Reentranz sorgen.
    fn write_str(&self, s: &str);
}

static SINK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Registriert die zusaetzliche Textsenke (Framebuffer-Konsole).
/// Die serielle Ausgabe laeuft davon unabhaengig weiter.
pub fn set_sink(sink: &'static dyn TextSink) {
    // Fat-Pointer koennen nicht atomar gespeichert werden; wir legen den
    // Trait-Objekt-Zeiger deshalb in einer statischen Zelle ab.
    unsafe {
        SINK_OBJ = Some(sink);
    }
    SINK.store(1 as *mut (), Ordering::Release);
}

static mut SINK_OBJ: Option<&'static dyn TextSink> = None;

#[inline]
fn sink() -> Option<&'static dyn TextSink> {
    if SINK.load(Ordering::Acquire).is_null() {
        None
    } else {
        // Nach dem Release-Store ist SINK_OBJ gesetzt und wird nie wieder geaendert.
        unsafe { core::ptr::addr_of!(SINK_OBJ).read() }
    }
}

struct AllWriter;
struct SerialWriter;

impl Write for AllWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::arch::serial_write(s);
        if let Some(k) = sink() {
            k.write_str(s);
        }
        Ok(())
    }
}

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::arch::serial_write(s);
        Ok(())
    }
}

/// Interne Ausgabefunktion fuer `print!`/`println!` (serielle Konsole + Sink).
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let _ = AllWriter.write_fmt(args);
}

/// Interne Ausgabefunktion fuer `serial_print!`/`serial_println!`.
#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    let _ = SerialWriter.write_fmt(args);
}

/// Ausgabe auf alle registrierten Konsolen.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::kcore::print::_print(format_args!($($arg)*)));
}

/// Wie [`print!`] mit Zeilenumbruch.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Ausgabe ausschliesslich auf die serielle Konsole.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::kcore::print::_serial_print(format_args!($($arg)*)));
}

/// Wie [`serial_print!`] mit Zeilenumbruch.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}

/// Strukturierte Boot-Logzeile: `[ ok ] subsystem: text`.
#[macro_export]
macro_rules! klog {
    ($sub:expr, $($arg:tt)*) => {
        $crate::println!("[karst] {:<10} {}", $sub, format_args!($($arg)*))
    };
}
