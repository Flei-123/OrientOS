# userland/ — was im Produkt-ISO liegt

**Diese Rolle ist seit dem 26.08.2026 eine andere.** Bis dahin stand hier
**ein** unprivilegiertes Programm: `hello.asm`, 64 Zeilen Assembler, dazu
ein eigenes Archivformat (`IRFS0002`, `mkinitramfs.py`), das der alte
Rust-Kernel als Bootloader-Modul einlas. Kernel, Archivformat und
`hello.asm` sind gelöscht ([KERNELWECHSEL.md](../KERNELWECHSEL.md) § 7);
in der Historie stehen sie weiter (`git show 91182a0:userland/hello.asm`).

Was an ihre Stelle getreten ist, ist deutlich mehr: **Osums Userland** —
eine Shell mit Röhren und Umlenkung und fünfundzwanzig Werkzeuge, alle in
Firn, alle eigenständige ELF-Dateien. Sie werden im **Osum-Repo**
geschrieben und gebaut; **dieses** Verzeichnis entscheidet, was davon ins
Produkt kommt und was sonst noch auf der Wurzel liegt.

Das ist genau die Teilung, um die es nach dem Kernelwechsel geht:

| | |
|---|---|
| **Osum** | schreibt Kernel und Programme (`kernel/user/*.fi`) |
| **OrientOS** | stellt daraus ein Produkt zusammen — diese Datei, `userland/PROGRAMME`, `build.sh` |

---

## Wie das Userland ins ISO kommt

Ein ISO hat **keine Platte**. Osums Userland lag bis dahin in einem
OFS-Dateisystem auf einer Platte, die QEMU mit `-drive` hereinreicht —
für ein bootfähiges Abbild ist das nichts. Was ein Multiboot-Lader neben
den Kern legen kann, ist ein **Modul**.

Osums Runde K10 nimmt eines entgegen: `kernel/bootmod.fi` liest die
Modultabelle des Laders, rechnet die **CRC32** über das ganze Modul,
vergleicht sie mit `modcrc=` auf der Kommandozeile und mountet es als
Wurzelplatte. Für `fs.fi` ändert sich dabei nichts — es sind dieselben
512 Oktette je Block, nur aus einem anderen Gerät.

```
./build.sh
  │
  ├─ vendor/osum/hole-osum.sh   Kernel + alle Programme aus dem
  │                             festgenagelten Commit bauen
  ├─ userland/PROGRAMME         welche davon ins Produkt kommen
  ├─ vendor/osum/mkfs.py        daraus ein OFS-Abbild bauen
  ├─ crc32                      Prüfsumme rechnen
  └─ limine.conf                module_path + "modfs modcrc=…"
```

---

## `userland/PROGRAMME`

Die ganze Zusammenstellung steht in einer Datei. Zeilenformen:

| Zeile | Bedeutung |
|---|---|
| `<name>` | ein Programm aus `vendor/osum/bin/` nach `/bin/<name>` |
| `verzeichnis <pfad>` | ein leeres Verzeichnis anlegen |
| `datei <zielpfad> <quelle>` | eine Datei aus **diesem** Repo hineinlegen |
| `bloecke: <n>` | Größe des Dateisystems in Blöcken zu 512 |

Ein Name, den es in `vendor/osum/bin/` nicht gibt, lässt `build.sh`
abbrechen — nicht stillschweigend weglassen. Ein Produkt, dem ein
Programm fehlt, von dem alle denken, es sei drin, ist schlimmer als eines,
das gar nicht baut.

`userland/dateien/` ist der Ort für das, was **OrientOS** beisteuert.
Zurzeit liegt dort `ausgabe.txt`; `test.sh` liest sie im laufenden System
mit `cat` zurück und zählt ihre Zeilen mit `wc -l` gegen die Zahl auf dem
Wirt. Das ist der Nachweis, dass dieses Repo dem Produkt wirklich etwas
hinzufügt und nicht nur weiterreicht, was Osum baut.

---

## Was daraus werden soll

Der Weg ist in [PACKAGING.md](../PACKAGING.md) beschrieben und beginnt
genau hier. Die Reihenfolge ist nach Abhängigkeit sortiert:

1. **Jetzt:** eine feste Liste von Programmen, ein Dateisystem, ein
   Modul. Genau ein Wurzeldateisystem, unveränderlich, für jede Marke
   dasselbe.
2. **Als Nächstes:** die Liste wird **markenabhängig**
   (`userland/PROGRAMME.<marke>`), damit sich zwei Produkte nicht nur im
   Namen unterscheiden. Der Quelltext bleibt derselbe
   ([BRANDING.md](../BRANDING.md)).
3. **Danach:** aus der Liste wird ein **Paketindex**. Jedes Programm
   bekommt seinen Inhalts-Hash, das Abbild bekommt `/store/<hash>/` und
   `/apps/<name> -> /store/<hash>` — die vier Regeln aus PACKAGING.md § 1.
   Das Wurzeldateisystem des ISOs ist dann nichts anderes als ein Store
   mit einer Generation.
4. **Offen, und zwar bewusst:** das Modul ist **nur lesbar in dem Sinn,
   dass Änderungen den Lauf nicht überleben** — es ist eine RAM-Platte.
   Für ein System, das sich selbst installiert, braucht es einen
   Schreibpfad auf eine echte Platte (Osum kann ATA und NVMe) und einen
   Partitionstabellen-Leser (Osum kann keinen). Das steht in
   [ROADMAP.md](../ROADMAP.md), nicht hier.

## Was der Lader von einem Programm verlangt

Das entscheidet nicht mehr dieses Repo, sondern Osums ELF-Lader
(`kernel/elf.fi`). Kurz: ELF64, little-endian, `ET_EXEC`, x86-64,
**statisch gebunden** — der Kern bindet nicht. Jedes `PT_LOAD` muss
`p_filesz <= p_memsz` erfüllen, ganz im unprivilegierten Adressbereich
liegen, sich mit keinem anderen Segment überlappen und nie zugleich
beschreibbar und ausführbar sein (W^X).

Wie streng das wirklich ist, misst `tests/step-60-elf-korpus.sh`: 53 von
Hand gebaute, jeweils genau einfach kaputte ELF-Köpfe liegen im
Produkt-ISO unter `/f/`, die Shell versucht in **einem** Boot jedes davon
zu starten, und der Kernel muss alle 53 überleben. Der Korpus stammt aus
der Zeit des Rust-Kernels (`tests/firn-elf/faelle/`) und ist das einzige
Stück davon, das nicht gelöscht wurde — die Fälle sind besser als der
Code, der sie hervorgebracht hat.
