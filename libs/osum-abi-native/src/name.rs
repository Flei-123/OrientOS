//! Namen fuer Eintraege in einem Namensraumknoten.
//!
//! Ein Name ist in dieser ABI **kein Pfad**. Er gilt ausschliesslich innerhalb
//! GENAU EINES Namensraumknotens, den der Aufrufer als Handle vorzeigt. Es gibt
//! keinen Wurzelknoten, den ein Prozess "einfach so" hat, und damit auch keinen
//! globalen Pfad-Namensraum: Wer keinen Namensraumknoten uebergeben bekommen
//! hat, kann keinen Namen aufloesen.
//!
//! Daraus folgen die Regeln unten. Sie stehen hier in der ABI-Crate (und nicht
//! im Kernel), damit sie host-getestet sind und eine spaetere Userspace-Bindung
//! dieselbe Pruefung benutzt.

use crate::{Error, Result};

/// Groesste Laenge eines Namens in Bytes. Bewusst klein: Namen sind
/// Bezeichner, keine Datenstruktur. Die Grenze begrenzt auch den Speicher,
/// den ein Aufrufer im Kernel durch Anlegen von Eintraegen belegen kann.
pub const NAME_MAX: usize = 32;

/// Prueft einen Namen und liefert ihn als `&str`.
///
/// Erlaubt sind 1..=[`NAME_MAX`] Bytes aus `a-z A-Z 0-9 . _ -`. Verboten sind
/// insbesondere:
/// * `/` und `\` — es gibt keine Pfade, also auch keine Trenner,
/// * `.` und `..` als vollstaendiger Name — kein "nach oben" aus einem Knoten,
/// * Steuerzeichen und alles jenseits von ASCII — Namen sollen in einem
///   Diagnoselog eindeutig darstellbar sein.
pub fn check(bytes: &[u8]) -> Result<&str> {
    if bytes.is_empty() || bytes.len() > NAME_MAX {
        return Err(Error::InvalidArgs);
    }
    for &b in bytes {
        let ok = b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-';
        if !ok {
            return Err(Error::InvalidArgs);
        }
    }
    if bytes == b"." || bytes == b".." {
        return Err(Error::InvalidArgs);
    }
    core::str::from_utf8(bytes).map_err(|_| Error::InvalidArgs)
}

/// Vergleicht zwei Namen. Gross-/Kleinschreibung ist bedeutsam — zwei Namen,
/// die sich nur darin unterscheiden, sind zwei verschiedene Eintraege.
#[inline]
pub fn eq(a: &[u8], b: &[u8]) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_are_accepted() {
        for n in ["a", "hello", "log.txt", "eine-datei_2", "A0", &"x".repeat(NAME_MAX)] {
            assert_eq!(check(n.as_bytes()), Ok(n), "Name {n} faelschlich abgewiesen");
        }
    }

    #[test]
    fn empty_and_overlong_names_are_rejected() {
        assert_eq!(check(b""), Err(Error::InvalidArgs));
        let zu_lang = "x".repeat(NAME_MAX + 1);
        assert_eq!(check(zu_lang.as_bytes()), Err(Error::InvalidArgs));
        // Genau an der Grenze muss es noch gehen.
        assert!(check("x".repeat(NAME_MAX).as_bytes()).is_ok());
    }

    #[test]
    fn there_are_no_paths_in_this_abi() {
        // Der Kern der Aussage "kein globaler Pfad-Namensraum": ein Trenner
        // ist kein gueltiges Zeichen, ein Name kann also nie mehr als einen
        // Knoten adressieren.
        for n in [
            "/", "a/b", "/etc/passwd", "..", ".", "../geheim", "a\\b", "C:\\x", "a b", "a\tb",
        ] {
            assert_eq!(check(n.as_bytes()), Err(Error::InvalidArgs), "{n} muss abgewiesen werden");
        }
    }

    #[test]
    fn control_and_non_ascii_bytes_are_rejected() {
        for b in [0u8, 1, 9, 10, 13, 27, 0x7f, 0x80, 0xff] {
            let name = [b'a', b];
            assert_eq!(check(&name), Err(Error::InvalidArgs), "Byte {b:#x} durchgelassen");
        }
        // Erschoepfend: jedes einzelne Byte einmal als ganzer Name. Der Punkt
        // allein ist ein gueltiges Zeichen, aber kein gueltiger NAME.
        for b in 0u8..=255 {
            let erlaubt = b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
            assert_eq!(check(&[b]).is_ok(), erlaubt, "Byte {b:#x} falsch bewertet");
        }
    }

    #[test]
    fn comparison_is_case_sensitive() {
        assert!(eq(b"log", b"log"));
        assert!(!eq(b"log", b"Log"));
        assert!(!eq(b"log", b"log2"));
    }
}
