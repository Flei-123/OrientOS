# Roadmap of OrientOS

Goal: an operating system of its own — no fork, no adopted foreign code,
kernel and system in [Firn](LANGUAGE.md). Time horizon: years. Sorted by
dependency, not by attractiveness.

Legend: **done** · **partial** · **open**

What runs today is in [README.md](README.md) under "What it can do" and
"What it cannot do". This file says what is still to be done.

**State of the figures: 2026-08-26, Osum `c5fe12f`.** Every figure in this
file comes from a run that was really carried out — the kernel figures from
the runners of the respective round (`tools/k13/run.sh`, `tools/k14/run.sh`,
`tools/k15/run.sh`, `tools/k16/run.sh` in the Osum repository, logged in its
`docs/ROUNDK*.md`), the line counts recounted with `wc -l` on the tree of
`c5fe12f` and not taken over from the logbooks. Where a figure from a
logbook differs from the recounted one, the recounted one is what stands
here.

**What these figures are NOT: a total.** The four rounds K13 to K16 were each
measured for themselves against the same 1486 checks of `main`, in four
separate runs. `./test.sh` in the Osum repository has therefore **not** been
run in one go over the merged state, and an added-up total would be a claim.
What stands here are the figures per round.

---

## 1 — Making the system usable

Without these items Osum is a demo system, not a working tool.

| | What | State |
|---|---|---|
| 1.1 | **Editor** — full-screen, raw terminal mode, search/replace, undo. Without it no file on the system can be changed. | **done** (K11, `/bin/edit`) |
| 1.2 | **Toolbox** — `find`, `sed`, `diff`, `patch`, `tar`, `gzip`, `xargs`, `du`, `top`, `mount`, `cut`, `tr`, `tee` | **done** (K11, 20 tools measured against their GNU counterparts) |
| 1.3 | **Shell scripts** — `if`, `for`, `while`, `case`, functions, variable substitution, `test` | **done** (K11) |
| 1.4 | **Users and permissions** — `uid`/`gid`, `chmod`, `chown`, `setuid`, login, `passwd` | **done** (K13). The ids live in the task record and are inherited in `sched.create`, so equally for `fork`, `execve`, `SYS_OSUM_SPAWN` and `proc.create`. The permission check is **one** function (`kernel/perm.fi`, 285 lines, `may_ids`), called from five gates — `open`, `mkdir`, `unlink`, `chdir`, `execve`; there is no second version of the rule in the tree. OFS carries `mode`, `uid`, `gid` in the inode. Tools in ring 3: `id` (120 l.), `whoami` (30), `chmod` (192, octal **and** letter form), `chown` (104), `su` (114), `passwd` (115), `login` (123). 103 declared system calls (79 + 24). Measured: `tools/k13/run.sh` **99 checks, 0 failures**; in the run 177 permission questions, 2 of them denied, the setuid bit taking effect 6 times |
| 1.5 | **`init` and services** — a first process that starts services, watches them, restarts them and cleans up on shutdown | **done** (K13). The kernel starts `/sbin/init` as process 1 instead of the shell (`kernel/kmain.fi`, line 1819 ff.); `init` (474 l.) reads `/etc/inittab`, starts services, restarts crashed ones, adopts orphans and shuts down through real ACPI. Plus `svc` (137 l.) for inspecting and controlling. The escape hatch is deliberately there and named: if there is no `/sbin/init` on the disk, the kernel takes `/bin/sh` — otherwise every image from the rounds up to K12 would have become unbootable (`kernel/kstate.fi`, word `initsh`) |
| 1.6 | **VFS** — mount several file systems side by side | **done** (K14). A file system registers with **nine** operations (`lookup readdir attr read write create unlink rename trunc`) plus a mask saying which of them it really can do; `vfs.fi` checks the bit **before** it uses the pointer. These are real indirect calls (`call *%rax`) — Firn stage 0 can neither turn a function pointer into a number nor call one out of a table field, but it can call a struct field. Files: `vfsops.fi` 138 l. (the shape), `mnt.fi` 327 (mount table, 8 entries of 128 bytes), `vfs.fi` 530 (the namespace), `ofs.fi` 178 (OFS as a driver of this layer), `procfs.fi` 1226 (`/proc`, produced in memory), `devfs.fi` 486 (`/dev` as a real file system). With that the special call `SYS_OSUM_PSTAT` is gone as well, the one `ps` and `top` used to ask through. Measured: `tools/k14/run.sh` **152 checks, 0 failures** |
| 1.7 | **FAT32** — so that Osum reads disks another system has written | **done, and more than asked for** (K14): `kernel/fat.fi`, 2007 lines, 93 functions, **reading AND writing**, with long names; measured against `mkfs.vfat`, `mcopy`, `mdir` and `fsck.fat`. Plus `kernel/part.fi` (498 l.): MBR and GPT with **both** CRC32 checksums — before that Osum looked in block 0 for an OFS superblock and found a partition table on every foreign disk. Named limits: only 512 bytes per sector (a FAT16 is refused), names up to 63 characters, a NEW name only in US-ASCII, at most 16 simultaneously open nodes, no `fsync` |
| 1.7b | **ext4 (reading)** — the second part of the old item 1.7, split off here deliberately | **open**. In the kernel tree of `c5fe12f` there is **not one line** about it: `grep -ril ext4 kernel/` finds nothing. This is not a subclause of 1.7 but a file system of its own, with extents, a journal and feature bits |

### Die Deckel, die in keiner Roadmap standen (OFS3 / MEM, 27.08.2026)

Beide Punkte hier haben **gefehlt** — nicht als „offen" eingetragen,
sondern gar nicht. Beide sind am 27.08.2026 am Baum gefunden worden und
nicht in einem Logbuch, und beide waren Groessenordnungen und keine
Feinheiten. Sie stehen deshalb hier, mit den Zahlen aus den Laeufen und
mit dem, was sie NICHT beweisen.

| | Was | Stand |
|---|---|---|
| 1.8 | **Die Platte darf groesser sein als zwei Megaoktett.** OFS hatte EINEN Block Blockkarte: 512 Oktette sind 4096 Bits, also 4096 Bloecke, also **2 MiB je Platte** — seit Runde 62, und der Quelltext hat es selbst gesagt („wer mehr will, braucht eine mehrblockige Karte, das ist eine andere Runde"). Dahinter sass ein zweiter Deckel: `kmain.fi` meldete die Wurzelplatte mit der Konstanten `OSUM_BLOCKS = 4096` an, **ganz gleich was dranhing** — eine Platte von 32 MiB lief nach 2 MiB voll. | **fertig** (Osum, Zweig `ofs3`, Commit `e2ee235`). Fassung 3 des Formats: mehrblockige Karte aus dem Superblock, Inode 128 → **256** Oktette (drei Zeitstempel, dreifach indirekter Zeiger), Verzeichniseintrag 32 → **264** (Name 23 → **255** Zeichen), `T_LINK` als vierte Inodeart, `rename` als echte Verrichtung. `blk.capacity` stellt jetzt ATA IDENTIFY. Gemessen (`tools/ofs3/run.sh`, **75 Zusagen, 0 Fehler**): groesste Platte 2 MiB → **4 GiB** (`df` im Gastsystem: `blocks total=8388608 free=8385271`), groesste Datei 2 134 016 → **136 351 744** Oktette (geschrieben und an drei Stellen zurueckgelesen: 2 396 160), Datei an Block **201252** gelesen UND beschrieben, Zeitstempel nach Neustart Zeichen fuer Zeichen derselbe. Aufwaertskompatibel: Fassungen 1 und 2 booten weiter (`ofsver=1`, `ofsver=2`), ohne `--v3` baut `mkfs.py` Oktett fuer Oktett dieselbe Platte wie vorher. **NICHT bewiesen:** nur 4 GiB gemessen (Format und LBA28-Treiber koennten 128 GiB), die 4-GiB-Platte ist eine Loch-Datei und nie vollgeschrieben, der dreifach indirekte Zeiger ist umgesetzt aber nie wirklich gebraucht worden, `rename` ueberschreibt nicht (POSIX verlangt es) und braucht `vfs` auf der Befehlszeile, harte Verweise gibt es aus Ring 3 nicht. [docs/ROUNDOFS3.md](../osum/docs/ROUNDOFS3.md) |
| 1.9 | **Der Arbeitsspeicher darf groesser sein als 512 Megaoktett.** Die Rahmen-Bitmap lag fest im Kerndatenbereich (`kstate.BITMAP_BYTES = 0x4000`), und die Rahmenzahl war `BITMAP_BYTES * 8` = 131 072 = 512 MiB — gemessen mit `-m 512`, `-m 2G`, `-m 8G` und `-m 64G` stand in ALLEN VIER Laeufen dieselbe Zahl. Und darueber hat der Kernel nicht bloss Speicher verschenkt, **er ist gestorben**: `boot.s` baut EINE Seitentafel zu 512 mal 2 MiB, also genau ein Gibioktett Identitaetsabbildung, und mit `-m 2G` endete der Lauf in `*** EXCEPTION 14 #PF err=0x0 cr2=0x7ffe1adc` — den ACPI-Tafeln knapp unter 2 GiB. | **fertig** (Osum, Zweig `ofs3`, eigener Commit). Drei Schritte, und die Reihenfolge ist die Sache: (1) die kleine Karte im Kerndatenbereich wie bisher, damit es ueberhaupt einen Zuteiler gibt; (2) die Identitaetsabbildung mit Rahmen aus Schritt 1 vergroessern — ein Rahmen je Gibioktett, PDPT ueber `cr3` gefunden statt ueber ein Symbol; (3) die grosse Karte aus Schritt 1 belegen, die 16 KiB von Schritt 1 darueberkopieren und erst dann den Rest eintragen. Gemessen (`tools/mem/run.sh`, **50 Zusagen, 0 Fehler**, ein Kernelabbild, nur `-m` anders): `-m 64G` → **17 039 360 Rahmen verwaltet, 16 775 326 frei, 2 129 920 Oktette Karte, 65 GiB abgebildet**, Lauf endet mit `kernel: done`. Ein Rahmen an **0x103ffff000** (65 GiB) beschrieben und zurueckgelesen. 200 000 Rahmen genommen, jeder beschrieben, jeder geprueft, jeder zurueckgegeben — freie Rahmen vorher = nachher in jeder Groesse. Auf 512 MiB unveraendert (131 072 Rahmen, 16 384 Oktette Karte). Kosten: 32 KiB Karte je GiB, bei 64 GiB **0,0034 %**. **NICHT bewiesen:** alles in QEMU/TCG, nichts auf echter Hardware; zwischen 64 und 512 GiB ist nichts gemessen (`MAX_GIB = 512`, ein PDPT); die Identitaetsabbildung wird als schreibrueckstellend zwischengespeicherte 2-MiB-Seiten gebaut, ohne MTRR/PAT; von den 16 Millionen Rahmen sind 200 000 wirklich angefasst worden und genau EINER am oberen Ende; nichts in diesem System FRAGT je nach so viel Speicher. [docs/ROUNDMEM.md](../osum/docs/ROUNDMEM.md) |

**Und ein dritter Deckel, der dabei aufgefallen ist und noch steht:** die
Kernelhalde ist 256 KiB aus 64 zusammenhaengenden Rahmen (`kmain.fi`),
erster Treffer, keine Groessenklassen. Das ist eine eigene Runde.

| | Was | Stand |
|---|---|---|
| 1.10 | **Kernelhalde** — 256 KiB fest, `kalloc`/`kfree` erster Treffer ohne Groessenklassen. Bei 64 GiB Arbeitsspeicher ist das dieselbe Sorte Missverhaeltnis wie 1.9 vorher. | offen |

## 2 — Oberflaeche

| | What | State |
|---|---|---|
| 2.1 | **Mouse** (PS/2, later USB HID) | **done** (K10, PS/2 on IRQ 12) |
| 2.2 | **Window server** — create, move, stack windows, focus, event delivery, recompose only the dirty regions. Through capability handles. | **done** (K10) |
| 2.3 | **Real fonts** — read and rasterise TrueType, antialiasing, kerning. The 8x16 font is only good enough for a console. | **done** (K10, TrueType with antialiasing, checked per character against a second rasterisation) |
| 2.4 | **Widget library** — buttons, lists, scroll bars, text fields, menus | **done** (K15), and specifically **in ring 3**: `kernel/user/wlib.fi` 2843 lines (widgets, layout, event loop) and `kernel/user/wlibc.fi` 862 (canvas, primitives, glyphs, colour scheme) against **499** lines of seam in the kernel (`kernel/wig.fi`, seven calls 1800..1806 that know no widget). The canvas is a **strip** of 800 x 81 pixels (259,200 bytes from `mmap`) and not the window buffer — a process has 832 KiB here between `brk` and `mmap`, a window of 640 x 420 needs 1,075,200 bytes. What is measured is **per character** and not per surface (the lesson from K7B, where text was "87 percent" right while every single letter was missing). Counter-check `nodirty`: 18,966,648 instead of 40,800,000 recomposed pixels for the same click |
| 2.4a | **NO graphical interface designer.** Explicitly rejected on 2026-08-26. The widget library is used like Windows Forms WITHOUT the designer: assemble windows in the source text, no drag surface, no property window, no code generator. It stands here so that the question does not come back in half a year. | rejected |
| 2.5 | **Terminal window** — the shell in a window instead of on the whole surface | **done** (K10, `/bin/sh` runs in it) |
| 2.6 | **File explorer** — `/bin/explorer`, displayed as File Explorer, second name `/bin/files`. NO proper name: the name IS the description, as in Windows. Plus a program directory `/apps/<name>.prog/` (bundles as with Apple, but NOT `.app` — too much Apple) and an instant search across the whole file system modelled on Everything. | **done** (K15). `kernel/user/explorer.fi` 1013 lines; `/bin/files` is a **second directory entry pointing at the same inode**, not a second copy. Bundles: `kernel/user/appdir.fi` 503 l., five `.prog` directories with `INFO`, `start`, `symbol` (format OSYM, 12 byte header) and `daten/`; `start` is again a second name for `/bin/<prog>`, so five bundles for 896 KiB of programs cost 5 x 1036 bytes of icon and **not one** block of code. Launcher with keyword search: `starter.fi` 443 l. Name index by the Everything method: `kernel/nidx.fi` 191 l. (journal ring in kernel memory, written in exactly two places — `fs.dir_add` and `fs.dir_remove`), `kernel/user/nidx.fi` 531 l., `suchen.fi` 356 l. The figures are below in a table of their own. Measured: `tools/k15/run.sh` **251 checks, 0 failures**, 31 QEMU runs, 26 of them with a screenshot |
| 2.6b | **The name index, in figures** — it stands here and not in a footnote, because it is the only claim of this group that asserts an order of magnitude | **measured** (K15, `tools/k15/gross.py`, image with 4000 empty files in seventeen folders, `--inodes=4096`). See the table below this group |
| 2.6c | **Speicherplatzanalyse** — `/bin/speicher` als Fenster und `/bin/du` auf der Kommandozeile, nach dem Vorbild von TreeSize, aber **ohne Durchlauf beim Start**. Am 27.08.2026 von Justin gewuenscht („so ein Tool wie TreeSize, nur besser und bereits integriert“). Moeglich, weil der Namensindex aus 2.6b schon steht: er traegt seit dieser Runde auch die **Groesse**, aufsummiert bis zur Wurzel, nachgefuehrt vom Aenderungsjournal des Kernels. | **fertig** (Runde SPEICHER, Zweig `speicher`). `kernel/user/speicher.fi` 909 Zeilen (Baum links, groesste Dateien rechts, Treemap nach „slice and dice“ unten, Loeschen aus dem Programm heraus), `kernel/user/du.fi` auf **denselben** Index umgestellt — zwei Wege mit verschiedenen Zahlen waeren ein Fehler. Der Journalring von 56 auf **127** Saetze vergroessert und in zwei eigene kdata-Seiten gezogen, weil seit dieser Runde **jeder `write`, der eine Datei waechst**, ein Satz ist. Zahlen unten in der eigenen Tabelle. Logbuch: Osum `docs/ROUNDSPEICHER.md` |
| 2.7 | **Settings, themes, wallpapers** — colour scheme as a file, load an image, a program for it. Legwork, as soon as the window server stands. | **done** (Osum, round DESKTOP + addendum TASKBAR, branch `taskbar-edge`). `/bin/einstellungen`, five pages: *Darstellung* copies a scheme out of `/etc/schemas/` to `/etc/theme` and a picture to `/etc/hintergrund`, and since the addendum also sets the taskbar; *Bildschirm* writes `/etc/schirm.conf`; *Zeit* writes the display offset to `/etc/zeit.conf`; *Netz* reads and sets the address through `osum_netget`/`osum_netset` or starts `/bin/dhcp`; *Benutzer* writes `/etc/shadow` through the same PBKDF2-HMAC-SHA256 as `/bin/passwd`. **Honest limits, and they stand in the window itself and not in small print:** this kernel sets the video mode at boot and cannot change it while running, and it can read the hardware clock but not set it. **A defect found and fixed on the way:** none of the drop-downs in this program had ever opened — `wlib` fires `K_CHOICE` with index `0x1000` and leaves the menu to the application, and the application did not. Resolution, time zone and DHCP were pictures of drop-downs. |
| 2.7b | **Optischer Feinschliff der Oberflaeche** — am 27.08.2026 von Justin beanstandet: die Bildschirmfotos sehen schlecht aus. Belegt und nicht geraten: der Bildschirm laeuft mit **800x600** (Fenster, Titelleisten und Schrift wirken dadurch grob), das Farbschema hat nur **160 bis 178** verschiedene Farben, und ein einziger Blauton (47,95,156) traegt die gesamte Hervorhebung. Zu tun: hoehere Aufloesung ueber GOP/VBE (mindestens 1280x800), groessere Schriftgrade, gleichmaessige Abstaende und Randbreiten, ein durchdachtes Farbschema statt gewachsener Einzelwerte, Fensterrahmen und Titelleiste neu gezeichnet. | offen |
| 2.8 | **Task management** — `top` on the console, then graphical: processes, memory, load, terminating | `top` done (K11), graphical open |
| 2.9 | **Desktop and taskbar** — there was no panel, no dock and no taskbar anywhere in the tree; every hit on „Leiste“ was the TITLE bar of a window. Asked for by Justin on 27.08.2026. | **done** (Osum, round DESKTOP + addendum TASKBAR, branch `taskbar-edge`). `/bin/schreibtisch` (a window on layer 0 that never comes forward, gradient or picture from `/etc/hintergrund`), `/bin/leiste` (start button, one button per window with switch-and-minimize, network, battery and clock out of system calls and out of nothing else). The window server got what made both possible: **three layers** (desktop / ordinary / always-on-top), **minimize** as distinct from close (`F_HIDDEN` — the window lives on, its buffer stays), windows **without decoration**, and `MAX_WIN` from 8 to 16. `WM_LIST`/`WM_ACT` answer only to a process that has reserved a screen edge, so an ordinary application does not get to see other processes' window titles. |
| 2.9b | **The taskbar has a position** — bottom, top, left or right, set by dragging it there or in the settings. Windows 10 could do this and Windows 11 dropped it. | **done** (Osum, addendum TASKBAR, branch `taskbar-edge`, `docs/ROUNDTASKBAR.md`). Left and right are a **vertical** bar with a different layout, not a rotated rectangle: buttons stack downwards, and the status fields measure their own text and **wrap** rather than clip. Both ways of setting it write the same `/etc/taskbar.conf` (`edge`, `height`, `width`, `autohide`, `ontop`) and the bar re-reads it twice a second, so a change in the settings arrives without a restart. Dragging shows a preview **rectangle** — four thin always-on-top windows, because a filled 128×600 window is 300 kilooctets and the process has 832 for everything. Measured (`tools/desktop/run.sh`, **102 assertions, 0 failures**, 11 QEMU boots, 8 screenshots): all four edges rectangle-exact with **0 pixels overlap and no gap**, **6654 inked pixels of text checked per character, 0 wrong**, the drag writes the file and the file is read back **out of the disk image**, the setting **survives a restart**, auto-hide comes back in **60 ms** (tick resolution 10 ms). Counter-test `nostrut`: the work area is the whole screen again and the maximized window covers the bar by **22 400 pixels**. |
| 2.9c | **Work area (`_NET_WM_STRUT`)** — the window server has to know which screen edge is occupied, or a maximized window sits under the taskbar. | **done** (Osum, addendum TASKBAR). One word in the window record (`W_STRUT = (edge+1) | (size<<8)`), four scalars for the result, and one rule: whoever changes a strut, a screen size or the visibility of a strut-holding window calls `recalc_work`, and `recalc_work` refits every maximized window. Nobody caches the work area anywhere else, and the runner checks that the copy the taskbar reads back through `WM_INFO` is the same one the server holds. `WM_STRUT` (2112) needs root; **reading** the work area (`WI_WORKX..WI_WORKH`) needs nothing but a handle. **This is the interface the round TILING needs**: the root rectangle of its frame tree is `work_x/y/w/h`, not the screen, and it changes at exactly one moment. |
| 2.9d | **Every window title on the screen was empty** — found on 27.08.2026, older than the addendum that found it. | **fixed** (Osum, addendum TASKBAR, one line). Round DESKTOP moved the window table out of `WM_OFF` into `DSK_OFF` and changed `wat()` and `set_title()` — but `title_text()` kept adding `WM_OFF` on top of an offset that already carried `DSK_OFF` and read zeros. Three assertions in `tools/wm/run.sh` said so in plain words („LEER: K l i c k m i c h“) and the round shipped anyway. Measured: `main` 103 passed / 0 failed, round DESKTOP 100 / **3**, this branch 103 / **0**. |

### The name index against the directory walk (K15)

An image with **4000 empty files** in a tree of seventeen folders, measured
in the same process: first the index, then the same search term as a
recursive tree walk.

| | |
|---|---:|
| names from the inode table | **4021** |
| system calls for building it | 65 (63 records per call) |
| time for building it | 820,123 µs, i.e. 203 µs per name |
| truncated | 0 |

| word | hits | index | tree walk | names differing | faster by |
|---|---:|---:|---:|---:|---:|
| `kupfer` | 1 | 6,966 µs | 1,966,404 µs | **0** | **282x** |
| `07` | 179 | 9,228 µs | 1,766,744 µs | **0** | 191x |
| `quaste` | 0 | 6,892 µs | 1,804,693 µs | **0** | 261x |

**Why 282 stands here and not 332.** The brief for this catch-up round said
"factor 332". In the logbook of the round (Osum `docs/ROUNDK15.md`, section
6c) stand 282, 191 and 261 — depending on the search word. What is entered
is what was measured. What the runner pins down is not the microseconds
anyway, but the **hit counts** and the **equality of the names**: both ways
deliver not just the same number but the same names, sorted and compared
byte for byte (`ungleich=0`). The times vary by about a fifth with the load
of the machine, the order of magnitude does not.

### Der Groessenindex gegen den Verzeichnisdurchlauf (SPEICHER)

Dieselbe Anordnung wie oben, aber ein Abbild, in dem die Dateien **Inhalt
haben** (`tools/speicher/baum.py`): 4000 Dateien in siebzehn Ordnern,
davon 308 mit Inhalt, 419 868 Oktette unter `/daten`. Leere Dateien
taugen fuer einen Namensindex, fuer eine Speicherplatzanalyse nicht —
alle Summen waeren null, und ein Fehler in der Aufsummierung fiele nicht
auf, weil 0 + 0 immer 0 ergibt.

| | |
|---|---:|
| Namen aus der Inode-Tabelle | **4030** |
| Aufbau des Index, einmalig | 1 365 386 µs (1,37 s) |
| **Abfrage aus dem Index** (`/daten`) | **13,4 µs** (zehn Abfragen in 134 µs) |
| **Vollstaendiger Durchlauf** (`/daten`) | **220 661 633 µs** (220,66 s) |
| Abfrage gegen Durchlauf | **16 467 286x** |
| **Aufbau** gegen Durchlauf | **161,6x** |
| Verzeichnisse gegengerechnet | **21 von 21**, Oktett fuer Oktett |
| Aufbau der ganzen Anzeige in `/bin/speicher` | 5 882 µs |

**Die letzte Zeile der oberen Haelfte ist die ehrlichere.** Der Index muss
ja auch erst entstehen. Er entsteht ueber `fs.scan` — die Inode-Tabelle
am Stueck, genau der Trick, mit dem WizTree schnell ist —, und schon
dieser Aufbau ist **161-mal** schneller als der Durchlauf. Er passiert
**einmal**, nicht bei jeder Frage.

**Drei Zahlen, nicht zwei.** Index und Durchlauf stehen in derselben
Datei und lesen dieselben Inodes; ein Denkfehler in der Regel, *was*
gezaehlt wird, stuende in beiden gleich falsch drin. Deshalb rechnet
`tools/speicher/baum.py` dieselben Summen ein drittes Mal auf dem **Wirt**,
in Python, ohne Kenntnis des Kernels. Alle 21 Verzeichnisse stimmen in
allen drei Wegen Oktett fuer Oktett ueberein.

**Dass das Journal Wachstum mitbekommt, ist der Kern der Sache** — nicht,
dass es neue Namen mitbekommt. Gemessen: 80 Bloecke zu 512 Oktetten in
eine neue Datei geschrieben ergeben **81 Journalsaetze**, `lost=0`, und
die Wurzelsumme bewegt sich um **genau** 40 960 Oktette (1 101 656 →
1 142 616); das Loeschen bringt genau den alten Stand zurueck; der Index
wurde im ganzen Lauf **einmal** gebaut. Die Gegenprobe ohne Nachziehen
steht daneben (`erwartet=1 142 616`, `nachher=1 101 656`, `ok=0`) — also
zog vorher wirklich das Journal.

**Was das NICHT ist: ein Vergleich mit TreeSize.** Gemessen wurde Osum
gegen Osum. Dass TreeSize durchlaufen *muss*, ist eine Aussage ueber
seinen Aufbau, keine Messung an seinem Programm. Und die Zeiten gelten
fuer QEMU ohne KVM — auf echter Hardware waeren beide kleiner. Die
uebrigen Einschraenkungen (harte Verweise ungeprueft, Index fasst 4200
Namen, nur 308 Dateien mit Inhalt) stehen in `docs/ROUNDSPEICHER.md`,
Abschnitt „Was NICHT bewiesen ist“.

**Und die Gegenprobe war zuerst kaputt, das gehoert dazu.** Der
Baumdurchlauf merkte sich anfangs *jeden* gelesenen Namen und war nach
sechzig Namen voll — er stieg dann in kein Unterverzeichnis mehr ab und
fand zu wenig. Ausserdem war `getdents64` quadratisch (31 000
Leseoperationen fuer 250 Dateien), der erste Messlauf ueber 4000 Dateien
lief zehn Minuten und war nicht fertig. Eine Gegenprobe, die man gewinnt,
weil man dem Gegner ein Bein stellt, ist keine; beides ist repariert,
bevor die Zahl oben entstand.

**What the index CANNOT do**, so that the figure does not promise more than
it covers: it survives no restart (the journal lies in kernel memory,
rebuild 0.94 s for 4021 names), the journal ring holds 56 records and
reports `lost` when it overflows, above 4200 names it reports "truncated",
and it holds **names, not paths**.

## 3 — Devices

| | What | State |
|---|---|---|
| 3.1 | **USB** — xHCI-Hostcontroller, Geraeteklassen (Tastatur, Maus, Massenspeicher), Anstecken im Betrieb. Der dickste Brocken dieser Gruppe, und Voraussetzung fuer fast jede echte Maschine. | **PLATZHALTER** |
| 3.2 | **Ton** — AC97 oder Intel HDA, Mischer, Lautstaerke, ein Abspielprogramm | offen |
| 3.3 | **Beruehrungsbildschirm** — ueber USB-HID oder I2C; sinnvoll erst nach 2.2 und 3.1 | offen |
| 3.4 | **Grafikkarte** — heute nur der lineare Rahmenpuffer des Bootladers. Modussetzung und Beschleunigung waeren die naechste Stufe. | offen |
| 3.5 | **Weitere Platten** — AHCI/SATA neben NVMe | offen |

## 4 — Power and performance

Not "make it faster", but make it **controllable** — like the power profiles
of Windows or Zorin: **power saving · balanced · maximum performance**.

| | What | State |
|---|---|---|
| 4.1 | **Taktverwaltung (P-States)** — Frequenz und Spannung ueber `IA32_PERF_CTL`/`HWP` setzen; darauf die drei Profile. Das ist der Kern der Sache. | **in Arbeit: Runde K18** (Osum, Zweig `k18-power`, abgezweigt von `c5fe12f`). Stand am 26.08.2026, 15:32: `kernel/pwr.fi` mit 755 Zeilen und `tools/k18/msrprobe.s` mit 496 Zeilen im Arbeitsbaum, noch nicht eingecheckt. **Keine gemessenen Zahlen** — und bei diesem Punkt ist das besonders zu betonen: eine Taktverwaltung, die nicht auf echter Hardware gemessen ist, hat nichts gezeigt |
| 4.2 | **Ruhezustaende (C-States)** — im Leerlauf wirklich schlafen statt zu drehen; `mwait` statt Warteschleife | in Arbeit (K18) |
| 4.3 | **Turbo** — die hoechste Stufe freigeben oder sperren, mit Blick auf die Temperatur | in Arbeit (K18) |
| 4.4 | **Akku und Netzteil** — Ladestand, Ladezustand, Restzeit, Netzteil an/ab ueber ACPI; Anzeige und Warnung | offen |
| 4.5 | **Temperatur und Luefter** — auslesen, drosseln bevor es heiss wird | offen |
| 4.6 | **Bildschirm** — Helligkeit, Abschalten nach Untaetigkeit | offen |
| 4.7 | **Bereitschaft** — Standby und Ruhezustand (S3/S4). Aufwaendig, weil jeder Treiber mitspielen muss. | offen |
| 4.9 | **Firn soll SCHNELLER werden als Rust** — nicht gleichauf, sondern davor. Ausdrueckliches Ziel von Justin, 27.08.2026. **Runde SPEED gelaufen** (Firn-Repo, Zweig `speed`, Logbuch `docs/ROUNDSPEED.md`, elf Runden). **Stand jetzt: Median 1,67x** statt 2,08x (`bench/RESULTS.md`, 9 Laeufe, zwei Durchlaeufe, beide Seiten identische Arbeit mit `black_box`): **sieve 1,01x** (war 4,16x) · bytecount 1,30x · fib 1,64x · statemachine 1,71x · bubblesort 1,84x · **matmul 2,01x**. Mit EINGESCHALTETEN Pruefungen (`release-safe`, Firn leistet also mehr als Rust) sind es **1,52x**. **Der groesste einzelne Posten war ein Messfehler und kein Codegen-Problem:** die alten Zahlen wurden auf `dev-fast` gemessen — der Standardstufe seit Runde 72, die jede Ganzzahloperation PRUEFT und nicht inlined — und gegen `rustc -O` gestellt; `bench/bench.py` benennt seither in jeder Spalte die Bauform. Der Rest kam aus dem Uebersetzer: Blocklayout, Schleifen in einem Stueck, exakte Schleifentiefe, Division durch eine Konstante ohne `div`, eine Bereichsanalyse (Preis der Sicherheit jetzt **0,98x**, war 1,97x vor Runde 90), `lea` fuer die Skalierung und die Bool-Zellen von `&&`/`||`, die seit Runde 92 im Speicher lagen (`jsonscan` -44,5 %, danach nochmal -11,6 %). **Ziel der Runde war Median unter 1,5x und sieve unter 2,5x:** sieve deutlich erreicht, der Median knapp nicht. Was jetzt `matmul` traegt, ist benannt und nicht geraten: es werden **keine Vektorbefehle** erzeugt, und die **Registerzuteilung ist linear statt faerbend**. Der Vorteil bleibt: der Uebersetzer gehoert uns ganz — es gibt keine fremde Zwischensprache, an der die Optimierung endet. | **Runde 1 fertig** (2,08x → 1,67x), Ziel Median <1,5x offen |
| 4.10 | **Der Kernel ist NICHT gegen Linux gemessen** — und das gehoert hierher, damit es niemand fuer erledigt haelt. Es gibt keine Zahl zu Systemaufruf-Durchsatz, Kontextwechselzeit oder Ein-/Ausgaberate im Vergleich zu Linux. Auf der Messmaschine (QEMU/TCG ohne `/dev/kvm`) waere eine solche Zahl auch wertlos: TCG uebersetzt Befehle, es taktet nichts. Braucht echte Hardware. | offen |
| 4.8 | **Messen statt raten** — Zwischenspeicher fuer Dateisystembloecke, `mmap` statt Kopieren, Auslagern, Zeitgeberaufloesung. Erst sinnvoll, wenn es genug Last gibt, die man messen kann. | offen |

## 5 — Foreign programs

| | What | State |
|---|---|---|
| 5.1 | **Linux binary compatibility** — statically linked programs through a mapping of the system call numbers. The ELF loader stands, and the POSIX layer was deliberately built with the Linux numbers. That is weeks, not years. | open |
| 5.2 | **Dynamic linking** — a loader for libraries, so that programs not statically bound run as well | open |
| 5.3 | **Hypervisor** — AMD-V (VT-x open), nested page tables, guest machines. With it foreign systems run unchanged; the benefit goes far beyond Windows (isolation, Linux guests, testing Osum in Osum). | **done for AMD-V** (K12: VMCB, NPT, guests, handles from ring 3). VT-x open — not checkable on the measuring machine (AMD EPYC without /dev/kvm) |
| 5.4 | **Windows programs** — through 5.3 in a guest system. A reimplementation of the Windows interfaces of our own (the Wine way) is explicitly NOT planned: Wine has been working on it since 1993 and has 783 of about 990 functions for `user32.dll` alone. | later |

## 6 — Software for the system

| | What | State |
|---|---|---|
| 6.1 | **Paketverwaltung** — unveraenderliche, inhaltsadressierte Pakete, drei Datentoepfe, Systemgenerationen | **gebaut** (26.08.2026, Zweig `pkg`). Format `.opk` und die Abweichungen vom Entwurf: [PAKETE.md](PAKETE.md). Werkzeug `pkg/opk.py` mit `bauen zeigen installieren entfernen liste aktualisieren generationen zurueck aufraeumen pruefen baum verweise`. Gemessen (`tests/step-80-pakete.sh`, 35 Zusagen, 0 Fehler): zweimal gebaut ergibt Oktett fuer Oktett dasselbe Paket (237 075 Oktette); ein Paket mit beschaedigten Daten, beschaedigten Metadaten **oder** veraendertem Hash im Kopf wird abgelehnt, das unversehrte angenommen; nach Installieren und Entfernen ist der Baum Oktett fuer Oktett der von vorher (23 Eintraege verglichen, dazwischen 16 veraendert); eine Generation zurueck stellt `apps/` Eintrag fuer Eintrag wieder her (13 Eintraege). **Seit 27.08.2026 laeuft `opk` AUF OSUM** (Osum-Zweig `install`, Commit `ddb681a`): `/bin/opk`, `kernel/user/opk.fi`, 1314 Z. Firn in Ring 3, dazu `sha.fi` (260 Z.) fuer SHA-256 beliebiger Laenge. Store, PLAN-Dateien und `/system/AKTUELL` genau wie oben — kein zweites Format. Neu sind zwei Systemaufrufe, `SYS_LINK` (86) und `SYS_SYNC` (162); **keine neuen Modusbits**. Gemessen auf dem gebooteten System (`tools/install/run.sh`, 70 Zusagen, 0 Fehler): ein Paket, das der WIRT gebaut hat und das Osum nie gesehen hat, wird installiert und **laeuft** aus `/apps`; `opk liste` nennt genau den Hash, den der Wirt gerechnet hat (`b7ee223324e20b36`); aktualisieren aus der Quelle nimmt die neue Fassung (`cec208d4dc40c2f4`), nach einem **Neustart** laeuft sie; zwei Generationen; `opk zurueck`, und die alte Fassung laeuft wieder. Gegenproben: ein gekipptes Oktett im Paket wird abgelehnt, das unversehrte im selben Lauf angenommen. **Und der Stromausfall:** QEMU mit SIGKILL mitten im Schreiben abgeschossen, an sechs Stellen zwischen 300 und 3600 ms — **6 von 6** starten danach, die Liste nennt immer genau eine der beiden Fassungen, nie etwas dazwischen. **Offen bleibt:** `opk.py` auf dem Wirt und `/bin/opk` sind zwei Umsetzungen desselben Formats; kein Aktualisieren des Kerns |
| 6.1a | **The self-bearing PLAN** — the system state must describe a whole machine, not just its applications | **built** (27.08.2026, branch `plan2`). Typed PLAN lines `app` / `kernel` / `source` / `setting` / `account`, old `name<TAB>hash` files still readable; the kernel is a package with `kind=kernel` and rolls back with `zurueck`; nine settings render the files Osum really reads (`/etc/netz.conf`, `/etc/zeit.conf`, `/etc/schirm.conf`); accounts carry the **SHA-256 of the credential**, never the credential. Measured (`tests/step-90-plan.sh`, 33 assertions, 0 failures): a 745-octet plan plus a signed source rebuilt a tree from nothing that is **identical over 39 entries, 18 files and 3 470 309 octets**; without the credentials exactly one file differs (`/etc/shadow`, accounts locked); a source signed by a foreign key is refused. Packages 3 → **69** (`pkg/recipes.py`, 62 recipes generated by machine, 7 skipped with a reason). **Open:** nothing boots from `system/kernel` yet — that is round INSTALL; no `https://` sources; the product ISO carries no settings. [docs/PLAN-FORMAT.md](docs/PLAN-FORMAT.md) · [docs/BACKUP.md](docs/BACKUP.md) · [docs/ROUND-PLAN2.md](docs/ROUND-PLAN2.md) |
| 6.1b | **The appearance is part of the system state, and it is a USER setting** — the answer to "if I set up a new device, does it look the same?" | **built** (27.08.2026, addendum to `plan2`). A sixth PLAN type `pref <user> <key> <value>` for colour scheme, wallpaper and taskbar edge/height/autohide, rendered to `/users/<who>/config/desktop/`; system-wide things (resolution, network, timezone) stay `setting` -> `/etc/`. Images are content-addressed: the plan carries the SHA-256, the octets are an **asset package** (`kind=asset`) in the same store, same signed INDEX, same collector -- and are generated from four lines of text (`pkg/osym.py` -> OSYM). Measured (`tests/step-91-look.sh`, 27 assertions, 0 failures): a system with another scheme, another wallpaper, the taskbar on the left and another timezone was rebuilt on an empty root **identical over 62 entries, 35 files, 4 396 976 octets**; the wallpaper has **three names and one inode**; with TWO accounts the `/etc` compatibility view **disappears**, because there is no honest answer to whose theme `/etc/theme` would be. Packages 69 -> **73**. **Open:** the product ISO carries no appearance (`/etc` collision with `userland/PROGRAMME`); nothing switches the view on login; `einstellungen.fi` still writes `/etc` directly, so a change made in the running system is not a generation. [docs/CONFIG-LEVELS.md](docs/CONFIG-LEVELS.md) |
| 6.1c | **Two machines** — `.opk` and `/apps/<name>.prog/` on x86-64 **and** AArch64 | **built** (27.08.2026, second addendum to `plan2`). A seventh PLAN type `arch <machine>` says what the MACHINE is; `arch=` in the package metadata says what the PACKAGE is for — two different facts, each written once, and the second is inside the hash, so a package cannot change what it claims without becoming a different package. **The architecture is measured, not declared:** `bauen` reads `e_machine` out of the ELF header of every file it packs, so the two recipes for two builds are identical except for one path, and a recipe that lies loses against the octets. What has no machine code in it gets `arch=any` — the same octets on both machines, so the same hash, so **one store entry serves both**. Separate packages per machine, **not** a fat one: a fat package's hash would name two things and the choice of half would be made outside the plan. Measured (`tests/step-92-arch.sh`, 35 assertions, 0 failures, real toolchain via `firnc --target=`): the same source runs on both machines printing the same output, two different hashes; a wallpaper packed on two hosts is **identical, 173 018 octets**; an aarch64 plan rebuilds on an empty root into ARM binaries and the two plans differ in **exactly 2 app lines plus 2 arch lines**; six refusals each with its counter-check (wrong machine, unlabelled package, ambiguous source, fat package, dependency across the boundary, a plan that names the other machine's build — which refuses **without half-building**); one store holds both machines while `/apps` names one. On the real 73-package source, `arch=any` saves **346 977 octets (2.9 %)** of a two-machine source — small today because only 4 of 73 packages are machine-independent. **Open:** Osum does not run on AArch64 yet (round OSUM-ARM), so the ARM binaries measured here are Firn's under `qemu-aarch64` and not Osum programs; all 69 code packages are `x86_64`; the product ISO has no architecture. [docs/ARCHITECTURES.md](docs/ARCHITECTURES.md) |
| 6.1d | **Programs with no source** — "what about applications that are not in the app store, when I set up a new device?" | **built** (27.08.2026, third addendum to `plan2`). A plan may name **any number** of sources, so the recommended answer is a **source of your own** (a directory with an INDEX and an Ed25519 signature, on a NAS or a stick) — measured: one `source-add` takes a tree from `2 of 3 covered, 1 ORPHANED` to `3 of 3, 0 ORPHANED`. For everything else, **orphanhood is decided by computation, not remembered**: `opk.py orphans` asks every recorded source whether it has each hash and names what no source can give back (`--strict` exits 1, so it can be a nightly check). Exactly those packages, and only those, are carried in the backup — `opk.py backup-set` emits the typed list round TRESOR needs (`plan`/`secret`/`tree`/`package`, plus `# unreachable` when a source could not be read), and `opk.py vault-export` writes the octets out as `.opk` files, refusing if repacking the store entry does not give back the very same hash. `rebuild --vault` fetches by hash and needs **no signature**, because a question that names a hash checks its own answer — unlike a source, which is asked for a *name*. Measured (`tests/step-93-orphans.sh`, 29 assertions, 0 failures): a hand-installed package is reported orphaned, exported as **1 package / 4 228 octets** that is the original file octet for octet, restored on an empty root so that the whole tree matches **entry for entry over 29 entries**, and without the vault the rebuild **refuses by name** (`MISSING mytool 5eff0f81b5cf2844`) having built **nothing**. `--allow-missing` builds the rest and leaves `system/INCOMPLETE` in the tree, which `verify` then **fails** on. An orphan for another architecture gets its own message — the octets are the only ones in the world and cannot run there, so the way out is the source code, not the backup. **Open:** no `https://`, so the recommended private source cannot be reached over a network; a backup restores the current state, not the history. [docs/BACKUP.md](docs/BACKUP.md) |
| 6.1e | **Moving a system to another machine on a stick** — "can I set a new device up from a backup WITHOUT signing in to an account?" | **built** (27.08.2026, fourth addendum to `plan2`). The answer is a principle and it is written down: **an account is a convenience, it is never a condition** — a sign-in server may make this easier, it may never make it possible, because it already is; the stick is the measure and the server is the special case. `opk.py stick-write` / `stick-restore`. The layout is **not a new format**: it is the backup set at the same relative paths plus `vault/`, and the step asserts every path `SET` names is really there; `vault/` also accepts **copied `store/<hash>/` directories** so round TRESOR can write what a block store wants — measured identical to the `.opk` form. **What a device is:** identity lives in `system/` **outside the generations**, so a plan cannot carry it even by accident (`machine-id` + Ed25519 device key, generated on every root and regenerated on every restore); `hostname` and a static address are dropped with a reason each unless `--keep-identity`, while `net.mode=dhcp` travels because it is a policy and not an address. Measured (`tests/step-94-move.sh`, 30 assertions, 0 failures) with the package source **moved out of the way** for the whole restore: an empty root became the old machine, compared **entry for entry over 64 entries including the documents** — the **only** differences are the PLAN (**921 → 768 octets**) and the two files those settings rendered; the counter-check with `--keep-identity` is **identical over all 64**, so the differences were exactly three and not roughly three. Three trees, **three distinct machine-ids**. `small` **219 213** vs `full` **2 170 942** octets (**9.9×**) — `full` is the default for a move because a move that needs a network is a download with extra steps, `small` stays the default for `backup-set`; an orphan is on both. Partial moves measured: `--no-personal` (3 programs, 0 documents, 0 accounts, 0 preferences, timezone stays) and `--no-data`. An x86-64 stick at an ARM device is refused with what **can** still be done. **Open:** nothing in Osum reads `machine-id` yet; a stick is not a bootable disk (round INSTALL); no user interface. [docs/MOVE.md](docs/MOVE.md) |
| 6.2 | **Paketquelle** — ein Ort, von dem installiert wird; Signaturen | **gebaut** (26.08.2026). Ein Verzeichnis mit den `.opk`-Dateien, einem `INDEX` und einer Ed25519-Signatur darueber. Ed25519 steht ZWEIMAL da — eigene Umsetzung nach RFC 8032 und die Bibliothek des Wirts —, beide rechnen bei jedem Pruefen, und bei Uneinigkeit bricht das Werkzeug ab. Gemessen: veraenderter Index abgelehnt, unveraenderter angenommen, ein Paket das nicht zum signierten Index passt abgelehnt. **Offen, und seit 27.08.2026 genauer gesagt:** (a) der oeffentliche Schluessel entsteht bei jedem Bau neu und ist damit keine Herkunft, sondern nur ein Verfahren; (b) **`/bin/opk` auf dem Geraet prueft die Ed25519-Signatur NICHT.** Es prueft, dass das Paket genau den Hash hat, den der `INDEX` ihm zuschreibt — das faengt ein ausgetauschtes **Paket**, aber keinen ausgetauschten **INDEX**. Wer die Kette ganz will, prueft die Quelle heute auf dem Wirt (`pkg/opk.py quelle`) und reicht sie geprueft herein. Ed25519 in Firn ist eine eigene Runde |
| 6.3 | **Certus** (der Browser; Befehl `/bin/browser`, Anzeigename "Certus", auf Android App-Name "Certus", Store-Titel "Certus Browser") — entsteht im Firn-Repo: HTML-Baumbau und DOM (94,89 % der html5lib-Tests), Layout mit Flexbox (31,72 % der Web Platform Tests), Zeichnen in Arbeit. Danach: JS an den DOM binden, HTTP, Oberflaeche. | in Arbeit |
| 6.4 | **Der Firn-Uebersetzer laeuft auf Osum selbst** — der eigentliche Pruefstein. Ein System, auf dem man sein eigenes System bauen kann, ist erwachsen; alles davor ist Kreuzuebersetzen von aussen. | **fertig** (K16). Osum hat auf sich selbst ein Firn-Programm uebersetzt, gebunden und ausgefuehrt, und das Ergebnis ist **Oktett fuer Oktett** dasselbe, was derselbe Uebersetzer auf Linux aus derselben Quelle macht: Assemblertext **14 909 Oktette zeichengleich**, ELF-Datei **8192 Oktette zeichengleich**. Dazu ein eigener Assembler und Binder in Firn, `kernel/user/fas.fi`, 2423 Zeilen — moeglich, weil der Uebersetzer nur einen kleinen Ausschnitt von x86-64 benutzt: ueber 1 533 513 Zeilen Assemblertext gemessen **58** Mnemoniken, **96** Paare aus Mnemonik und Operandenform, 8 Direktiven, 4 Speicherausdruecke, kein Indexregister, kein Massstab, kein Segment. `fas` bindet **54 von 54** Programmen des Userlands; 16 davon gegen `as`+`ld` gehalten, 16-mal gleiche Ausgabe. `.fi` ist doppelklickbar (`kernel/ftype.fi`, `kernel/user/firun.fi`, 242 Z.). Gemessen: `tools/k16/run.sh` **64 Zusagen, 0 Fehler** |
| 6.4a | **…aber `firnc` liegt noch nicht im OrientOS-Produkt.** Das ist die ehrliche Trennung zwischen „Osum kann es" und „das Produkt hat es" | **offen, und leicht zu uebersehen.** `vendor/osum/hole-osum.sh` baut die 69 Programme aus `kernel/user/*.fi`; darunter sind `fas` und `firun`, **nicht** aber `firnc` — der entsteht in K16 durch Querbauen von Firns `bin/firnc1.fi` gegen `kernel/user/user.ld` und braucht dafuer das Firn-Repo. Solange das nicht in `hole-osum.sh` steht, kann man auf einem gebooteten OrientOS ein `.s` assemblieren, aber keine `.fi` uebersetzen |

## 7 — Other machines

| | What | State |
|---|---|---|
| 7.1 | **Die Architekturgrenze** — x86-Details liegen im Kernel ueberall statt an einer Stelle. Der Rust-Kernel dieses Repos HATTE diese Grenze (Traits plus ein Testschritt, der x86-Begriffe ausserhalb von `arch/` verbot); sie ist **nicht** nach Firn portiert worden, weil das eine Umbauarbeit an jedem Modul waere und keine Portierung. Die Anforderung steht als unuebersetzte Vorlage im Baum: `vorlage/arch_iface.rs`. Der eigentliche Inhalt ist nicht der Trait-Text, sondern die Regel. Siehe [KERNELWECHSEL.md](KERNELWECHSEL.md) § 4.1. | offen, mit Vorlage |
| 7.1b | **aarch64** — Firn uebersetzt bereits nach ARM (296 von 300 Faellen auf beiden Maschinen identisch), der Kernel nicht. Setzt 7.1 voraus. | offen |
| 7.2 | **Die Geraetewirklichkeit auf ARM** — kein PCI-Erkennungsweg wie beim PC, sondern Geraetebaum, und jeder SoC ist anders. Der eigentliche Aufwand. | offen |
| 7.3 | **Certus als App auf Android** (App-Name "Certus", Store-Titel "Certus Browser", Paket-ID `com.orientos.certus`) — der schnellste Weg, das Ergebnis in die Hand zu bekommen. Android ist Linux plus Bionic; eine APK darf native Bibliotheken enthalten. Also: Browserkern nach `aarch64-linux-android` uebersetzen, als `.so` einpacken, duenne Huelle fuer Zeichenflaeche und Beruehrung. Voraussetzung: der Browser ist fertig genug und laesst sich als Bibliothek herausloesen — beim Bauen von Anfang an mitdenken. | offen |

## 8 — Security and hardening

| | What | State |
|---|---|---|
| 8.1 | **SMEP/SMAP** — Ring 0 fuehrt keinen Nutzercode aus und fasst Nutzerdaten nur im `stac`-Fenster an. Auf JEDEM Kern (CR4 ist pro Prozessor), zurueckgelesen statt behauptet. Gegenproben: `smapraw` gibt mit dem Bit einen #PF und ohne es das Oktett. | **fertig** (26.08.2026, Osum `kernel/guard.fi`) |
| 8.2 | **Speicherverwuerfelung (KASLR)**, Schutzseiten, nicht ausfuehrbarer Stapel | offen |
| 8.3 | **Signierte Pakete und signierter Start** | **halb** — signierte Paketquelle steht (6.2, Ed25519 ueber den Index, zwei Umsetzungen). Signierter Start ist offen, und er ist die schwierigere Haelfte: er braucht eine Vertrauenskette, die vor dem Kernel anfaengt |
| 8.4 | **Dauerlauf** — Tage statt Minuten, mit Blick auf Fragmentierung und Lecks | offen |

---

## 9 — What the kernel switch left open

The switch to the Osum kernel was completed with the cut on 2026-08-26
([KERNELWECHSEL.md](KERNELWECHSEL.md) § 7). Two items were **deliberately**
not ported, and both stand here so that they do not vanish into a footnote.

| | What | State |
|---|---|---|
| 9.1 | **Die Architekturgrenze** — dieselbe Sache wie 7.1, von der anderen Seite: nicht „wir wollen ARM", sondern „der Rust-Kernel konnte etwas, das der Firn-Kernel nicht kann". Vorlage: `vorlage/arch_iface.rs`, 378 Zeilen, nicht uebersetzt. | offen, mit Vorlage |
| 9.2 | **Kanaele, Ports, Namensraeume, `ProcessSpawn`, Speicherobjekte** der nativen ABI. Die Nummern (ab 2000), die Fehlerwerte und die Bedeutungen sind portiert und stehen in Osums `sys.fi`; die Objekte dahinter fehlen und antworten `NotSupported` (−9). `NotSupported` und nicht `ENOSYS`: der Aufruf EXISTIERT in dieser ABI, dieser Kernel bietet ihn nur nicht an. | offen |
| 9.3 | **Ein markenabhaengiges Userland.** Zwei Marken unterscheiden sich bisher nur im Namen. `userland/PROGRAMME.<marke>` waere der naechste Schritt — der Quelltext bliebe derselbe. | offen |
| 9.4 | **Ein Schreibpfad auf eine echte Platte.** Das Boot-Modul ist eine RAM-Platte; Aenderungen ueberleben den Lauf nicht. | **gebaut** (27.08.2026, Osum-Zweig `install`, Commit `ddb681a`). `/bin/install` (`kernel/user/install.fi`, 933 Z., Firn, Ring 3) schreibt Schutz-MBR, GPT-Kopf und Eintragstafel mit **beiden** CRC32 samt Sicherung am Plattenende, eine FAT32-EFI-Partition und eine OFS-Wurzelpartition, und kopiert das laufende Wurzeldateisystem aus dem Boot-Modul darauf. **Es braucht dafuer keinen neuen Systemaufruf**: `/dev/hda` ist seit K14 eine echte Datei mit `lseek`/`read`/`write`. Gemessen (`tools/install/run.sh`, **70 Zusagen, 0 Fehler**): 10 240 kopierte Sektoren, Wurzeldateisystem auf **452 207** Bloecke gewachsen, beide GPT-CRC32 mit `zlib` nachgerechnet, Sicherungstafel Oktett fuer Oktett die primaere, `fsck.fat` auf der EFI-Partition **0**, Kern (1 713 708) und Bootlader (253 952) Oktett fuer Oktett die gebauten. Danach startet die Maschine **ohne ISO und ohne Modul** ueber OVMF von der Platte (`osum: rootpart=1 first=72048`, `osum: from module` kommt nicht vor), und Schreiben ueberlebt den Lauf. Gegenproben: ohne `--ja` bleibt die Platte Oktett fuer Oktett unberuehrt; ein gekipptes Oktett in der GPT-Tafel, und der Kern lehnt sie ab (`part: gpt crc mismatch`, `parts=0`) — das musste ueber einen **rohen** Startweg gemessen werden, weil UEFI den primaeren GPT aus der Sicherung repariert, und dieser Befund steht als eigene Zusage daneben. Zwei Fehler, die aelter sind als die Runde, dabei gefunden: FLUSH CACHE an ein besetztes Laufwerk, und `blk.read` wiederholte nicht, waehrend `blk.write` es tat. **Offen:** keine echte Hardware, nur ATA (LBA28, also 128 GiB), kein NVMe-Installationsweg, kein Aktualisieren des Kerns. Logbuch: `osum/docs/ROUNDINSTALL.md` |

---

## 10 — The pinned kernel, and what had to be corrected in it

OrientOS does not build against the newest Osum but against **one** commit
(`vendor/osum/COMMIT`). On 2026-08-26 the pin moved from `7a53ac3` to
**`c5fe12f`** — four rounds and 32,525 lines in between
(`git diff --stat 7a53ac3 c5fe12f`: 103 files, 32,525 inserted, 267 deleted
lines).

**This commit does not compile.** That is not an aside but the reason why
there has been a patch stack in this repository since 2026-08-26. Two places
got lost in the merge of `k15-ui`:

| file | what is missing | what firnc says |
|---|---|---|
| `kernel/kmain.fi` | EINE schliessende Klammer in `mode_of` | `'fn' is only allowed at top level, not inside a function body` — 13 Funktionen stehen dadurch im Rumpf von `mode_of` |
| `kernel/sys.fi` | die K15-Naht (1800..1806) liegt in `k13_call` statt in `dispatch` | `unknown name 'a4'` — `k13_call` nimmt a0..a3. Und uebersetzt waere der Zweig **tot**: `is_k13` zaehlt 1800..1806 nicht auf |
| `tools/osum/mkfs.py` | `load()` ruft `Fs(blocks, inodes)` — die Signatur ist `(blocks, version, inodes)` | kein Abbruch, sondern etwas Schlimmeres: jedes Abbild mit mehr als 128 Inodes wird mit falscher Geometrie gelesen. `data_start` steht dann auf 34 statt 130, also **auf der Inodetabelle**. Lesen geht (ein Inode traegt absolute Blocknummern), Schreiben wuerde die Tabelle ueberschreiben. Aufgefallen ist es erst, als die Paketrunde ein Abbild mit 512 Inodes brauchte |

Die ersten beiden sind Merge-Verluste und keine Absicht: der Elternteil `78f8e86`
hat den `kmain`-Block vollstaendig, der Elternteil `fcb5c30` hat die Naht
in `dispatch`. Berichtigt wird es **hier** und nicht im Kernelrepo — ein
verschobener Nagel ist ein anderer Stand und entwertet die Messungen von
gestern. Die Dateien liegen in `vendor/osum/patches/`, `hole-osum.sh`
wendet sie beim Auspacken an und **bricht ab**, wenn eine nicht mehr
passt; `tests/step-05-patches.sh` nimmt jede einzeln wieder heraus und
verlangt, dass firnc den Kernel dann ablehnt.

**Independently confirmed.** The same two defects were found on the same day
by the two running kernel rounds, each on its own: `k17-usb` (commit
`93ad914`, "two merge defects from c5fe12f fixed, nothing builds otherwise")
and `k18-power` (commit `eed1edd`, "main would not compile — two leftovers
of the K15 merge"). Three findings, the same brace. As soon as one of these
branches goes to `main`, the patch no longer fits, the run turns red, and
the file belongs deleted — not adapted.

**And a figure that turned up in the process.** `userland/PROGRAMME` stood
at `bloecke: 6144`. The block map of OFS is ONE block of 512 bytes, so 4096
bits, so at most 4096 blocks per image — up to K15 `mkfs.py` did not check
that and built 6144 without complaint; the upper 2048 blocks were not
addressable. It was never noticed, because the product never needed them: 27
programs, 932,584 bytes, **1822 of 4062 data blocks**. Since K15 `mkfs.py`
refuses 6144, and that is as it should be.

---

## Standing rules

1. **No Linux code, no fork.** Rebuilding from specifications yes, adopting no.
2. **No claim without a measurement.** Every check has a **counter-check** — the same kernel with the property switched off, where the measurement has to break down. A green test without a counter-check does not count.
3. **Nothing is deleted before the replacement demonstrably runs.**
4. **What is missing is stated.** Gaps are named, not written away.
5. **What is deleted stays in the history.** `git rm`, never `rm`. A deleted
   kernel is not forgotten, it is only out of the way — and `./test.sh`
   checks that the history still knows it.

## Related design documents

| file | content |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | layers, architecture boundary, dependencies |
| [PACKAGING.md](PACKAGING.md) | immutable, content-addressed packages, system generations |
| [FILESYSTEM.md](FILESYSTEM.md) | VFS, FAT32, ext4 reading, a file system of its own |
| [LANGUAGE.md](LANGUAGE.md) | Firn: why a language of its own, and what it demands of the kernel |
| [KERNELWECHSEL.md](KERNELWECHSEL.md) | comparison module by module, what is ported and what is outstanding |
| [BRANDING.md](BRANDING.md) | one source text, several products |
