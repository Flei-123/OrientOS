# NAMEN.md — Namensfindung: Stand und Begründung

Stand: 25.08.2026 (OS-Name gesetzt; Kernelteil unveraendert vom 19.08.2026)

| Ebene | Name | Status |
|---|---|---|
| **Kernel** | **osum** | **gesetzt** (Cargo-Paketname `osum`) |
| **Betriebssystem** | **OrientOS** | **gesetzt** (25.08.2026, ersetzt den Arbeitstitel `Karstos`) |
| **Sprache** | **Firn** | gesetzt, eigenes Projekt |

Umbenennung erfolgt ausschließlich über `./rename.sh <kernel> <os>` — siehe
`RENAME.md`. Im Quelltext steht kein Produktname; alles kommt aus
`kernel/src/kcore/branding.rs`.

---

## Warum OrientOS

Gesetzt am 25.08.2026. **OrientOS** war der Name eines frueheren eigenen
Projekts (~2024): ein Betriebssystem von Grund auf mit eingebautem
KI-Assistenten. Der Kernel von damals ist nie geflogen, die Idee schon --
sie steht jetzt in [ASSISTENT.md](ASSISTENT.md). Der Name schliesst das ab.

### Geprueft am 25.08.2026

| Register | `orientos` |
|---|---|
| crates.io | frei (fuer den OS-Namen ohnehin unerheblich -- nur der **Kernel** ist ein Cargo-Paket) |
| npm | frei |
| PyPI | frei |

| Domain | Stand |
|---|---|
| `orientos.org`, `orientos.sh`, `orient-os.com` | **frei** |
| `orientos.com` | vergeben |
| `orientos.dev` | keine Nameserver gesetzt, beim Registrar zu pruefen |

**Vorbelastung:** `OrientDB`, eine Graphdatenbank, existiert weiter. Andere
Kategorie, keine Verwechslungsgefahr in der Domaene Betriebssystem -- anders
als bei den frueher verworfenen Kandidaten, die alle *in derselben Kategorie*
kollidierten. Auf GitHub gibt es kein ernstzunehmendes Betriebssystem dieses
Namens.

**Hinweis:** "Orient" ist ein geografisch aufgeladener Begriff. Bewusst
akzeptiert.

### Warum nicht `OsumOS`

Registerseitig frei, aber verworfen: Kernel `osum` und System `OsumOS`
unterscheiden sich um zwei Buchstaben. Kernel- und Produktname trennt man --
NT/Windows, XNU/macOS, Linux/GNU machen es alle so, und aus gutem Grund:
sonst laesst sich in keinem Satz mehr sagen, welche Ebene gemeint ist.

### Zweitmarke

Eine Variante **XoffiOS** ist vorgesehen. Sie wird **kein** eigener Zweig:
ein Quellbaum, zwei Builds. Dafuer bekommt `kernel/src/kcore/branding.rs`
zusaetzlich einen `option_env!`-Vorrang, wie ihn `brand.rs` in FreeViewer hat
-- dann laufen zwei Marken nebeneinander, ohne dass `./rename.sh` den Baum
anfasst. Unterschiede zwischen Marken gehoeren in **Daten** (welche Pakete im
Abbild liegen, Erscheinungsbild, Paketquelle), nie in Code.

---

## Warum osum

- **Alle drei relevanten Paketregister frei**: crates.io, npm, PyPI. Kein
  anderer geprüfter Kandidat hatte das.
- **Kurz, tippbar, aussprechbar**, keine Umlaute, keine Sonderzeichen.
- Wortspiel auf **„awesome"** — bewusst gewählt, nicht Zufall.

### Vorbelastung (geprüft, bewusst akzeptiert)

| Fund | Einschätzung |
|---|---|
| **OSUM** — Hobby-OS-Blogprojekt, 2008, „an 'Awesome' Operating System" | seit ~2008 inaktiv, keine Reichweite. Unkritisch |
| **Ossum Inc.** — Softwarefirma, Doppel-s | andere Schreibweise, anderer Markt |

### Domains (whois, 19.08.2026)

| frei | vergeben |
|---|---|
| `osum.cc`, `osum.sh`, `osumos.org` | `osum.com`, `.dev`, `.org`, `.io`, `.systems`, `osumos.com`, `osumos.dev` |

`osum.com` ist belegt und aktiv (HTTP 200). Für ein Open-Source-Projekt ohne
Marketing-Anspruch unerheblich — `osum.sh` passt zu einem Systemprojekt ohnehin
besser.

---

## Geprüfte und verworfene Kandidaten

### Wegen bestehender Software in derselben Domäne

| Name | Konflikt |
|---|---|
| **zircon / zirkon** | **Kernel von Google Fuchsia OS.** Identische Kategorie — schlechtestmögliche Kollision |
| **wolfram** | **`WolframKernel`** ist der dokumentierte Prozessname der Mathematica-Rechenmaschine. Zusätzlich Marke von Wolfram Research für Software |
| **vanadium** | **Browser von GrapheneOS** (gehärteter Chromium + WebView). Sicherheits-OS mit eigenem Browser = exakt dieses Projekt. Bekanntheit allerdings auf eine Nische begrenzt |
| **osmium** | **libosmium** (OpenStreetMap-C++-Bibliothek) + `osmium-tool`, `pyosmium`, `node-osmium`. Alle drei Register belegt. Zusatz: „Osmium" bedeutet wörtlich *Geruch* |
| **titan** | Google-Sicherheitschip, Saturnmond, dutzende Produkte |
| **radix** | Radix DLT (Krypto) + „radix sort" als Standardbegriff. Alle Register belegt |
| **solum** | OpenStack Solum, PyPI/npm belegt |

### Wegen belegter Paketregister

limen · cardo · silex · stratum · basalt · obsidian · serac · arx · vallum ·
nodus · urd · lithos · gaia · nadir · umbra · penumbra · syzygy · skarn ·
achat · rutil · kaldera · iridium · hafnium · palladium · rhodium · fortis ·
magnus · primus · imperium · robur · vis · hyperion · kronos · tartarus ·
aegis · bastion · ferrum

### Ernsthafte Alternativen, die frei blieben

| Name | Bedeutung | Anmerkung |
|---|---|---|
| **nunatak** | Felsgipfel, der aus dem Gletschereis ragt (grönländisch) | stärkstes Bild zu *Firn*, kein Konflikt gefunden. Zweitplatzierter |
| **tantal** | Metall, das Säuren widersteht und nichts aufnimmt | inhaltlich passend zur Isolation. npm belegt |
| **niob** | Metall, korrosionsfest | überall frei, aber unbekannt |
| **chthon** | „Erde, Tiefe" (altgriechisch) | härtester Klang |
| **gneis** | Gestein unter Hochdruck | `gneis.org` frei |
| **eklogit** | Gestein aus ~50 km Tiefe | `eklogit.org` frei |

---

## Erkenntnis für spätere Namensentscheidungen (OS-Name!)

**Das Periodensystem ist als Namensquelle abgegrast.** Sicherheits- und
OS-Projekte greifen seit Jahren darauf zu: Zircon, Vanadium, Osmium, Wolfram,
Titan, Iridium — alle vergeben, teils genau in dieser Domäne.

Prüfreihenfolge für den OS-Namen:
1. **crates.io + npm + PyPI** (schnell, harte Fakten)
2. **Websuche** auf bestehende Software — *nicht* aus dem Gedächtnis raten
3. **whois** für Domains
4. Markenregister, falls das Projekt je vermarktet werden soll

Maßstab ist nicht „gibt es das Wort irgendwo?", sondern
**„kollidiert es dort, wo dieses Projekt lebt?"**

---

## Kontrolle nach der Umbenennung

```
./rename.sh osum OrientOS     # ausgeführt 19.08.2026
./build.sh && ./run-qemu.sh --check
./test.sh                    # ALLE TESTS BESTANDEN
```
