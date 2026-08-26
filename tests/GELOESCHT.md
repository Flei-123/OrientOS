# Was gelöscht wurde — und wo es jetzt steht

Diese Liste ist **kein Nachruf, sondern ein Werkzeug.** `tests/step-70-doku.sh`
prüft, dass jeder Pfad, den die Dokumente dieses Repos nennen, entweder
**existiert** oder **hier steht**. Ein Verweis auf eine Datei, die es nicht
gibt, ist schlimmer als kein Verweis — er sieht überprüfbar aus und ist es
nicht. Wer etwas löscht, trägt es hier ein; wer das vergisst, macht den
Testlauf rot.

Alles wurde mit `git rm` entfernt. Die Historie hat jede Zeile davon;
die Commit-Angabe in der letzten Spalte ist der letzte Stand, in dem die
Datei noch da war.

## Der Kernelwechsel, 26.08.2026

Begründung, Abgleich und Zahlen: [KERNELWECHSEL.md](../KERNELWECHSEL.md).

| Weg | Wo es jetzt steht | Letzter Stand |
|---|---|---|
| `kernel/src/` (12 269 Zeilen Rust) | Osum, `kernel/*.fi` | `git show 586ac70:kernel/src/main.rs` |
| `kernel/src/abi/native.rs` | Handle-Modell: Osum `kernel/cap.fi`; Kanäle/Ports/Namensräume: **nicht portiert**, KERNELWECHSEL.md § 4.2 | `git show 586ac70:kernel/src/abi/native.rs` |
| `kernel/src/abi/posix.rs` | Osum `kernel/sys.fi` | `git show 586ac70:kernel/src/abi/posix.rs` |
| `kernel/src/arch/x86_64/user.rs` | SMEP/SMAP: Osum `kernel/guard.fi`; Ring 3: Osum `kernel/user.fi` | `git show 586ac70:kernel/src/arch/x86_64/user.rs` |
| `kernel/src/kcore/arch_iface.rs` | **`vorlage/arch_iface.rs`** — steht als unübersetzte Vorlage im Baum | — |
| `kernel/src/kcore/initramfs.rs` | Osum `kernel/bootmod.fi` | `git show 586ac70:kernel/src/kcore/initramfs.rs` |
| `kernel/src/kcore/mem.rs` | die fünf Typen stehen im Kopf von `vorlage/arch_iface.rs` | `git show 586ac70:kernel/src/kcore/mem.rs` |
| `kernel/src/drivers/fbcon.rs`, `font.rs` | Osum `kernel/fb.fi`, `font.fi`, `tty.fi` | `git show 586ac70:kernel/src/drivers/fbcon.rs` |
| `libs/osum-abi-native/`, `libs/osum-abi-posix/`, `libs/osum-mem/` (5 892 Zeilen) | Osum `cap.fi`, `sys.fi`, `mem.fi` | `git show 586ac70:libs/osum-abi-native/src/syscall.rs` |
| `kernel/firn/serial.fi`, `bitmap.fi`, `elf.fi` (922 Zeilen Firn) | Osum `serial.fi`, `mem.fi`, `elf.fi` | `git show 586ac70:kernel/firn/elf.fi` |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `kernel/Cargo.toml`, `kernel/build.rs`, `kernel/linker-x86_64.ld`, `x86_64-osum-none.json` | nichts — es wird in diesem Repo kein Rust mehr übersetzt | `git show 586ac70:Cargo.toml` |
| `run-qemu.sh` | `run-osum.sh` | `git show 586ac70:run-qemu.sh` |
| `limine.conf` | wird von `build.sh` erzeugt (mit `module_path` und `modcrc=`) | `git show 586ac70:limine.conf` |
| `userland/hello.asm`, `user.ld` | Osums Userland im Boot-Modul, siehe `userland/PROGRAMME` | `git show 586ac70:userland/hello.asm` |
| `userland/mkinitramfs.py`, `mkbroken.py` | `vendor/osum/mkfs.py` (OFS statt eigenem Archivformat) | `git show 586ac70:userland/mkinitramfs.py` |
| `tests/firn-bitmap/`, `tests/firn-fehler/` | Osums eigene Prüfstände (`tools/`) | `git show 586ac70:tests/firn-bitmap/pruefstand.c` |
| `tests/firn-elf/pruefstand.c`, `tests/firn-elf/tempo.c`, `tests/firn-elf/rust-massstab.rs`, `tests/firn-elf/lauf.sh`, `tests/firn-elf/lauf-rust.sh`, `tests/firn-elf/alter-pruefteil.rs.txt` | Osum `kernel/elf.fi` + `tools/osum/run.sh`; **die 53 Fälle bleiben** und laufen in `tests/step-60-elf-korpus.sh` | `git show 586ac70:tests/firn-elf/lauf.sh` |
| `tests/firn-fehler/TEMPO.md`, `README.md` | die Befunde stehen weiter in `../LANGUAGE.md` (L-*) | `git show 586ac70:tests/firn-fehler/TEMPO.md` |
| `tests/step-16-preempt.sh`, `tests/step-17-ring3.sh`, `tests/step-18-elf.sh`, `tests/step-19-handles.sh`, `tests/step-20-ring3-preempt.sh`, `tests/step-21-doku-verweise.sh`, `tests/step-22-firn-bitmap.sh`, `tests/step-23-firn-elf.sh` | Osums `./test.sh` misst den Kernel; hier messen `tests/step-*.sh` das System | `git show 586ac70:tests/step-16-preempt.sh` |
| `tests/verweise.py` | die Pfadprüfung in `tests/step-70-doku.sh` — sie prüft Pfade statt Zeilennummern, weil Zeilennummern verrutschen und Pfade nicht | `git show 586ac70:tests/verweise.py` |
| `tests/step-24-osum-kernel.sh`, `step-25-osum-caps.sh`, `step-26-kernelwechsel.sh` | aufgegangen in `step-20-produkt.sh`, `step-30-boot.sh`, `step-50-schutz.sh`, `step-70-doku.sh` | `git show 586ac70:tests/step-24-osum-kernel.sh` |
| `target/` | nichts — Bauverzeichnis eines Kernels, den es nicht mehr gibt | — |
