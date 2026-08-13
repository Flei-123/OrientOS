# userland/ — unprivilegierte Programme und Startdateisystem

Alles in diesem Verzeichnis wird von `build.sh` gebaut und landet als
**Bootloader-Modul** (`limine.conf`: `module_string: initramfs`) im ISO-Abbild.
Der Kernel findet es ueber `boot::limine::module_by_string("initramfs")` und
liest es in `kcore/initramfs.rs`; geladen wird es von `kcore/elf.rs`.

| Datei | Zweck |
|---|---|
| `hello.asm` | erstes unprivilegiertes Programm, nasm, statisch, ohne libc |
| `user.ld` | Linkerskript: zwei PT_LOAD-Segmente (RX ab `0x401000`, RW dahinter, `.bss` mit `memsz > filesz`) |
| `mkinitramfs.py` | packt die Dateien ins Archiv (Format unten) |
| `mkbroken.py` | erzeugt aus `hello` ein absichtlich kaputtes Abbild fuer den Negativtest |

Bauen (macht `build.sh` automatisch):

```sh
nasm -f elf64 -o build/userland/hello.o userland/hello.asm
ld -n --build-id=none -T userland/user.ld -o build/userland/hello build/userland/hello.o
python3 userland/mkbroken.py build/userland/hello build/userland/kaputt.elf
python3 userland/mkinitramfs.py build/initramfs.img hello=... kaputt.elf=... liesmich.txt=...
```

## Archivformat (`IRFS0001`)

Alles little-endian:

```text
0x00  8 B   Kennung "IRFS0001"
0x08  u32   Anzahl Eintraege
0x0c  u32   Gesamtlaenge des Archivs in Bytes
0x10  Tabelle, Anzahl * 48 B:
        +0x00  32 B  Name, mit Nullbytes aufgefuellt (kein '/', ASCII)
        +0x20  u64   Offset der Daten ab Archivanfang
        +0x28  u64   Laenge der Daten
danach: die Daten, jeweils auf 16 B ausgerichtet.
```

**Warum nicht cpio oder tar.** Beide transportieren POSIX-Metadaten (Modus,
uid/gid, mtime, Verzeichnisse, Symlinks, Pfade). Der Kern kennt davon nichts —
keine ambient authority, kein globaler Pfadnamensraum, keine Benutzerkennungen;
ein cpio-Leser muesste diese Felder parsen und sofort wegwerfen. Ausserdem sind
beide stromorientiert: der n-te Eintrag ist erst nach n-1 Koepfen mit
ASCII-Oktalzahlen erreichbar. Das Format hier ist eine Tabelle fester Breite;
der Kernel prueft jede Grenze mit `checked`-Arithmetik gegen die tatsaechliche
Bereichslaenge, bevor er ein einziges Byte liest.

## Aufrufkonvention von `hello`

Nummer in `rax`, Argumente in `rdi, rsi, rdx, r10, r8, r9`, Ergebnis in `rax`
(`>= 0` Erfolg, `< 0` Fehlerwert — kein `errno`). Nummern siehe
`libs/karst-abi-native/src/syscall.rs`. Beim Eintritt uebergibt der Kernel in
`rdi` das Handle, ueber das das Programm schreiben darf: ohne explizit
uebergebenes Handle kann es nichts ausgeben.

`hello` erfragt die ABI-Version, schreibt eine Zeile ueber dieses Handle,
beschreibt sein `.bss` (Nachweis, dass RW wirklich RW ist) und beendet sich mit
`ProcessExit`.
