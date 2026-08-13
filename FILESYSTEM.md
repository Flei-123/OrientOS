# Dateisysteme in Karstos

Reihenfolge und Begründung. Gebaut ist davon in Phase 1 noch nichts — dieses
Dokument legt fest, **in welcher Reihenfolge** gebaut wird und warum gerade so.

---

## Die Reihenfolge

| Schritt | Was | Warum an dieser Stelle |
|---|---|---|
| **1** | **VFS-Schicht**, objekt-/handle-basiert, FS-Treiber hinter einem Trait | ohne saubere Grenze zementiert der erste Treiber seine Semantik im ganzen System |
| **2** | **FAT32 lesen und schreiben** | Pflicht: die EFI System Partition ist FAT. Ohne FAT kein eigener Bootloader, kein Update des eigenen Systems auf UEFI |
| **3** | **ext4 nur lesen** | Interoperabilität: Daten von bestehenden Linux-Platten holen, ohne sie zu gefährden |
| **4** | **eigenes Dateisystem** (Arbeitstitel `karstfs`) | erst wenn alles darüber steht und man weiß, was man wirklich braucht |

---

## 1. VFS: objekt- und handle-basiert, **keine** POSIX-Semantik erzwungen

Der VFS ist die Stelle, an der die meisten Systeme sich POSIX für immer
einhandeln. Linux' `struct inode`/`struct dentry` sind nicht nur
Implementierung, sie sind ein **Vertrag**: jedes Dateisystem muss so tun, als
hätte es Inodes, Hardlinks, `..`-Einträge, `mode_t`-Bits, Zeitstempel in einer
bestimmten Auflösung und einen globalen Wurzelbaum. FAT hat nichts davon und
wird deshalb überall mit Emulationsschichten verbogen.

Karstos macht es andersherum:

* Ein Dateisystem liefert **Objekte hinter Handles**, keine Inodes.
* Die Trait-Grenze verlangt nur, was jedes Dateisystem wirklich kann:
  öffnen relativ zu einem Verzeichnis-Handle, lesen, schreiben, Größe,
  auflisten, erzeugen, löschen, umbenennen, `sync`.
* Alles Weitere ist **optional und abfragbar** statt vorausgesetzt:
  Hardlinks, Symlinks, erweiterte Attribute, Nanosekunden-Zeitstempel,
  Snapshots, Prüfsummen. Ein Treiber meldet seine Fähigkeiten; wer mehr will,
  fragt vorher.
* **Kein globaler Root.** Ein Prozess bekommt ein Namespace-Handle übergeben
  (siehe [PACKAGING.md § 7](PACKAGING.md)); es gibt keinen prozessunabhängigen
  Pfad `/`. `..` ist deshalb keine besondere Operation, sondern schlicht nicht
  vorhanden — man kann nicht aus einem Handle „herausklettern".
* **POSIX-Semantik lebt in der POSIX-Schicht.** Dort werden Pfade aufgelöst,
  dort gibt es `cwd`, `umask`, `mode_t` und `errno`. Fällt die Schicht weg,
  fällt die Semantik weg — der VFS merkt es nicht.

Konkret heißt das für den Trait (Entwurf, noch nicht implementiert):

```rust
pub trait FileSystem {
    fn capabilities(&self) -> FsCaps;                 // was kann dieses FS wirklich
    fn root(&self) -> NodeId;                         // Wurzel DIESES Dateisystems
    fn open(&self, dir: NodeId, name: &str, rights: Rights) -> Result<NodeId>;
    fn read (&self, node: NodeId, off: u64, buf: &mut [u8]) -> Result<usize>;
    fn write(&self, node: NodeId, off: u64, buf: &[u8])     -> Result<usize>;
    fn list (&self, dir: NodeId, cb: &mut dyn FnMut(&DirEntry)) -> Result<()>;
    fn create(&self, dir: NodeId, name: &str, kind: NodeKind) -> Result<NodeId>;
    fn unlink(&self, dir: NodeId, name: &str) -> Result<()>;
    fn sync(&self) -> Result<()>;
}
```

Kein `stat` mit 20 Feldern, von denen 15 gelogen sind. Wer Metadaten will,
fragt gezielt und bekommt `NotSupported`, wenn es sie nicht gibt.

---

## 2. FAT32 zuerst — nicht aus Nostalgie, aus Zwang

Die **EFI System Partition ist FAT**. Das ist keine Konvention, das steht in der
UEFI-Spezifikation. Ohne FAT-Schreibzugriff kann Karstos:

* seinen eigenen Bootloader nicht installieren,
* seine eigenen Systemgenerationen nicht in den Bootpfad eintragen,
* sich selbst nicht aktualisieren.

Dazu kommt: FAT ist auf jedem USB-Stick, in jeder Kamera, auf jeder SD-Karte.
Es ist der kleinste gemeinsame Nenner für Datenaustausch mit der Welt.

FAT ist gleichzeitig das perfekte **Gegenbeispiel** für den VFS-Entwurf: keine
Inodes, keine Hardlinks, keine Rechte, 8.3-Altlast mit LFN-Aufsatz,
Zeitstempel in 2-Sekunden-Schritten. Wenn der VFS-Trait FAT ohne Verrenkungen
trägt, ist er nicht heimlich POSIX. **FAT ist der Lackmustest.**

---

## 3. ext4 nur lesen — bewusst kastriert

Schreibender ext4-Support hieße: Journal korrekt führen, Extent-Bäume
umbauen, Orphan-Listen behandeln, `e2fsck`-Semantik nachbilden. Ein Fehler
darin zerstört fremde Daten — Daten, die dem Nutzer gehören und die er nicht
uns anvertraut hat, sondern Linux.

Lesend ist der Nutzen fast derselbe (Daten herüberholen) bei einem Bruchteil
des Risikos. Schreibzugriff kommt frühestens, wenn ein eigener Test-Korpus mit
`e2fsck`-Gegenprobe existiert — und ehrlich gesagt vermutlich nie, weil der
Nutzen den Aufwand nicht trägt.

---

## 4. Das eigene Dateisystem — zuletzt, und das mit Absicht

### Warum nicht zuerst?

Weil Dateisysteme die Komponente sind, in der Fehler **irreversibel** sind. Ein
abstürzender Kernel kostet einen Neustart; ein Dateisystem mit einem Fehler in
der Schreibreihenfolge kostet die Daten.

Zwei Beispiele, die niemand ignorieren sollte:

* **btrfs RAID 5/6.** Angekündigt, gebaut, benutzbar wirkend — und mit einer
  „write hole"-Schwäche behaftet, die bei Stromausfall während eines
  Stripe-Updates zu stillem Datenverlust führen kann. Die offizielle Warnung,
  es nicht produktiv zu benutzen, steht seit **über einem Jahrzehnt**. Es reicht
  nicht, dass ein Dateisystem im Normalfall funktioniert.
* **ZFS.** Von den ersten Entwürfen bei Sun (um 2001) bis zur breit
  vertrauenswürdigen Reife vergingen **Jahre** mit einem bezahlten Team, das
  nichts anderes tat. Prüfsummen, CoW, Snapshots, Resilvering — jedes einzelne
  Merkmal ist einfach zu beschreiben und schwer richtig zu machen.

Ein eigenes Dateisystem ist also kein Wochenendprojekt und darf nicht auf dem
kritischen Pfad zu „Karstos bootet und ist benutzbar" liegen. Bis dahin tun es
FAT32 und ein Initramfs.

### Anforderungen, wenn es dann gebaut wird

**Pflicht, weil das Systemmodell darauf steht:**

1. **Copy-on-Write mit Snapshots.** Ohne das gibt es keine Systemgenerationen
   und kein Rollback in Sekunden ([PACKAGING.md § 5](PACKAGING.md)). Das ist
   der Grund, warum CoW hier kein Zusatzmerkmal ist, sondern die Grundlage.
2. **Prüfsummen über Daten *und* Metadaten.** Stiller Datenverlust ist die
   schlimmste Fehlerklasse: er wird erst beim Backup-Zurückspielen sichtbar,
   wenn auch das Backup schon kaputt ist.
3. **Dedup über Inhalts-Hashes.** Der App-Store ist content-addressed; das
   Dateisystem darf denselben Block dann nicht mehrfach speichern.
   Blockweise, nicht dateiweise.
4. **Handle-orientiert**, passend zum VFS: kein Pfad-Namensraum als Grundlage,
   Verzeichnisse sind Objekte wie alles andere.

**Altlasten, die einmal sauber entschieden werden — und dann nie wieder:**

| Frage | Entscheidung |
|---|---|
| 8.3-Namen | gibt es nicht. Namen sind Namen. |
| Zeitstempel | 64-bit-Nanosekunden seit 1970, **kein 2038-Problem**, kein 2-Sekunden-Raster |
| Zeichensatz | UTF-8, **normalisiert bei der Erzeugung** (NFC), abgelehnt wenn ungültig |
| Groß-/Kleinschreibung | **case-sensitive, exakt**. Kein „case-preserving, case-insensitive" — das ist der Ursprung ganzer Fehlerklassen und macht Unicode-Vergleiche zur Zeitbombe |
| Trennzeichen | `/` ist im Namen verboten, `\0` ist verboten, sonst alles erlaubt |
| Maximale Namenslänge | 255 Bytes UTF-8, hart geprüft |
| Sonderdateien | keine Geräteknoten im Dateisystem. Geräte sind Handles, nicht Dateien |

Der Punkt bei dieser Tabelle: Jede dieser Fragen wird in bestehenden Systemen
**mehrfach und widersprüchlich** beantwortet (HFS+ vs. APFS vs. ext4 vs. NTFS).
Wer sie einmal entscheidet und dokumentiert, spart sich zwanzig Jahre
Sonderfallcode.

---

## 5. Reihenfolge im Verhältnis zur Roadmap

| Phase | Voraussetzung dafür |
|---|---|
| Initramfs (Phase 3) | nichts — liegt im Speicher, reicht für die ersten Userspace-Programme |
| VFS + FAT32 (Phase 4) | Blockgeräte-Trait, also AHCI/NVMe |
| ext4 lesend (Phase 4) | VFS steht |
| eigenes FS (Phase 8) | alles darüber steht, und es gibt einen Testkorpus samt Absturzsimulation |

Vor dem eigenen Dateisystem steht ein **Absturztest-Werkzeug**: künstliche
Stromausfälle an zufälligen Punkten der Schreibreihenfolge, danach Prüfung auf
Konsistenz. Ohne dieses Werkzeug wird nicht angefangen. Das ist die Lehre aus
btrfs RAID 5/6.
