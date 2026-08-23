// tests/firn-elf/rust-massstab.rs — faehrt die 53 Faelle aus faelle/ gegen die
// RUST-Fassung des ELF-Pruefteils.
//
// WOZU
//
// Bevor `parse()` nach Firn wandert, muss feststehen, dass der Massstab die
// ALTE Fassung wirklich beschreibt. Sonst misst man die Neufassung an einer
// Erwartung, die noch nie jemand gegen etwas Laufendes gehalten hat — und
// jeder Unterschied waere unentscheidbar: Fehler in der Portierung oder Fehler
// im Testfall?
//
// Deshalb laeuft dieses Programm ZUERST. Es bindet den Pruefteil aus
// `kernel/src/kcore/elf.rs` per `include!` ein — nicht als Kopie, sondern die
// echte Datei, aus dem Abschnitt zwischen den beiden Markierungen. Die drei
// Dinge, die der Pruefteil aus dem Kernel braucht (`is_user_addr`,
// `USER_LIMIT`, `PAGE_SIZE`), stehen hier mit denselben Werten.
//
// Uebersetzen und laufen lassen: tests/firn-elf/lauf-rust.sh

use std::fs;
use std::path::Path;

// --- die drei Dinge aus dem Kernel, mit den echten Werten -------------------
// kernel/src/kcore/user.rs: USER_BASE / USER_LIMIT
const USER_BASE: u64 = 0x0000_0000_0040_0000;
const USER_LIMIT: u64 = 0x0000_8000_0000_0000;
const PAGE_SIZE: usize = 4096;

const fn is_user_addr(addr: u64) -> bool {
    addr >= USER_BASE && addr < USER_LIMIT
}

// --- der Pruefteil, unveraendert aus dem Kernel ------------------------------
include!(concat!(env!("PARSER_QUELLE")));

// --- die Zuordnung Fehler -> Zahl. Muss mit faelle.py uebereinstimmen. -------
fn code(e: &ElfError) -> u32 {
    match e {
        ElfError::TooShort => 1,
        ElfError::BadMagic => 2,
        ElfError::BadClass => 3,
        ElfError::BadEndian => 4,
        ElfError::BadType => 5,
        ElfError::BadMachine => 6,
        ElfError::BadTable => 7,
        ElfError::SegmentOutOfFile => 8,
        ElfError::SegmentOutOfRange => 9,
        ElfError::SegmentOverlap => 10,
        ElfError::SegmentsSharePage => 11,
        ElfError::SegmentInsane => 12,
        ElfError::BadAlign => 13,
        ElfError::NeedsInterpreter => 14,
        ElfError::BadEntry => 15,
        ElfError::TooManySegments => 16,
    }
}

fn main() {
    let verzeichnis = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/firn-elf/faelle".to_string());
    let dir = Path::new(&verzeichnis);
    let liste = fs::read_to_string(dir.join("erwartet.txt"))
        .expect("erwartet.txt fehlt — erst tests/firn-elf/faelle.py laufen lassen");

    println!("Massstab gegen die RUST-Fassung (kernel/src/kcore/elf.rs)\n");
    let mut bestanden = 0usize;
    let mut gefallen = 0usize;

    for zeile in liste.lines() {
        let mut teile = zeile.splitn(3, ' ');
        let name = teile.next().unwrap();
        let erwartet: u32 = teile.next().unwrap().parse().unwrap();
        let beschreibung = teile.next().unwrap_or("");

        let bytes = fs::read(dir.join(name)).expect("Falldatei fehlt");
        let got = match parse(&bytes) {
            Ok(_) => 0u32,
            Err(e) => code(&e),
        };
        if got == erwartet {
            bestanden += 1;
            println!("  [ ok ] {name:<34} {beschreibung}");
        } else {
            gefallen += 1;
            println!("  [FEHL] {name:<34} lieferte {got}, erwartet {erwartet}  ({beschreibung})");
        }
    }

    println!("\n{bestanden} bestanden, {gefallen} gefallen");
    if gefallen > 0 {
        std::process::exit(1);
    }
}
