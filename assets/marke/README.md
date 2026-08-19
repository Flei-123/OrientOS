# Marke — osum

Flach, ohne Verlauf, ohne Schein. Alle Formen sind reine Geometrie
(Kreis, Bogen, Gerade), keine Schriftdatei nötig.

| Element | Datei |
|---|---|
| Wortmarke hell | `osum-wortmarke-<variante>.svg` |
| Wortmarke dunkel | `…-dunkel.svg` |
| Wortmarke einfarbig | `…-mono.svg` |
| Signet (nur das O) | `osum-signet-<variante>.svg` |
| Übersicht mit Größentests | `osum-logo-v3.png` |

## Varianten

| | Symbol im O | Bedeutung |
|---|---|---|
| **a-kern** | gefüllter Punkt | der Kern. Beste Erkennbarkeit bei 16 px |
| **b-ring** | Ring im Ring | Ring 0 innen, Ring 3 außen |
| **c-tor** | Punkt + schmale Lücke rechts | die Schwelle zwischen den Ringen. Nachteil: wirkt klein wie ein C |

## Farben

| | Hex |
|---|---|
| Schwarz | `#0B0D10` |
| Akzent (Cyan) | `#22D3EE` |

## Regeln

- **Kein Glow, kein Verlauf, kein Schatten.** Das Logo muss einfarbig,
  im Druck und bei 16 px funktionieren.
- Das **Signet ist das O allein** — für Favicon, App-Icon, Sticker.
- Cap-Höhe 100, Strichstärke 26. Optisches Kerning: O–S enger (6) als S–U (14).
- Untertitel `KERNEL` ist optional (`-nurwort` = ohne).

## Runde 2 — Nadel-Varianten (Wunsch: Kompass-Doppelnadel im O)

Uebersicht: `osum-nadel-v4.png` · Groessentest 16/24/32 px: `groessentest-nadel.png`

| Var | Symbol im O | Dateien | Bewertung (echter Groessentest) |
|---|---|---|---|
| D | Doppelnadel diagonal, cyan/schwarz | `s-d-nadel-diag.*`, `w-d-nadel-diag*` | 32 px klar als Kompassnadel lesbar, 24 px ok, 16 px verliert die Richtung. Bestes Bild bei grosser Darstellung |
| E | Doppelnadel senkrecht, cyan/grau | `s-e-nadel-senk.*` | einzige Variante, die bei 16 px noch Richtung zeigt. Robusteste Wahl fuers Favicon |
| F | Cursor-Pfeil einfarbig (Vorlage) | `s-f-cursor.*` | ab 24 px klar, bei 16 px nur noch ein cyan Fleck. Inhaltlich Navigation statt Kern |
| G | Doppelnadel diagonal, schmaler | `s-g-nadel-fein.*` | zu duenn, faellt schon bei 24 px auseinander. Raus |

Empfehlung: **D** als Hauptmarke (gross), **E** als Favicon/Kleinform.
Zweifarbige Nadel ist Pflicht — einfarbig liest sich die Doppelnadel als Raute, nicht als Kompass.

## Runde 3 — Strichstaerke + Nadeltypen (`logo_gen.py`)

**BUG GEFUNDEN:** Der S-Pfad aller Dateien aus Runde 1+2 ist kaputt — am zweiten
Bogen stand `sweep-flag 0` statt `1`, dadurch lief der untere Bogen in dieselbe
Richtung wie der obere und das S wurde ein ueberlappender Klecks. Behoben in
`logo_gen.py`. Alle Assets muessen neu erzeugt werden, sobald die Variante feststeht.

Ab jetzt ist alles parametrisch: `python3 logo_gen.py <strichstaerke> <nadeltyp>`

**Strichstaerke** (`osum-strichstaerken.png`, Versalhoehe = 100)

| sw | Urteil |
|---|---|
| 26 | erste Fassung, deutlich zu fett — die Punze im O wird so eng, dass die Nadel keinen Platz hat |
| 21 | kraeftig, noch schwer |
| **17** | **ausgewogen — Empfehlung.** Punze gross genug fuer die Nadel, Wortbild ruhig |
| 14 | leicht, elegant, bei kleinen Groessen schon duenn |
| 11 | zu fein fuer Favicon/Bootlog |

**Nadeltyp** (`osum-nadel-typen.png`)

| Typ | Beschreibung | Urteil |
|---|---|---|
| `mono` | beide Haelften cyan | liest sich als Raute/Linse, nicht als Kompassnadel |
| `mono_fuge` | beide Haelften cyan + feine Fuge | Nadel erkennbar, Fuge verschwindet aber bei 16 px |
| `duo_soft` | cyan / dunkleres cyan (#12A3BE) | **Empfehlung** — Nadel klar lesbar, ohne harten Kontrast |
| `duo_hard` | cyan / schwarz | staerkstes Kompassbild, dunkle Haelfte verschmilzt aber mit dem Ring |

Das M hat jetzt Breite 68 (statt 76) und einen tieferen Mittelknick (y=72 statt 63)
und liest sich damit als normales M — vorher war der Knick zu kurz und die
Miter-Spitzen bei sw=26 zu lang.


## Runde 4 — FESTGELEGT

Strichstaerke **17**, Doppelnadel **zweifarbig dezent**, Achse links-oben nach rechts-unten (40 Grad).
Obere Haelfte `#22D3EE`, untere Haelfte `#0D8299` (Stufe 4 aus `osum-nadel-abstufung.png`).

Erzeugt mit `python3 logo_gen.py` — alle Dateien unten sind Ausgabe dieses Skripts,
nichts ist von Hand nachgezogen. Aendern heisst: Konstante im Skript anpassen, neu bauen.

| Datei | Zweck |
|---|---|
| `osum-wortmarke-hell.*` | Standard, dunkle Schrift auf hellem Grund |
| `osum-wortmarke-dunkel.*` | weisse Schrift auf `#0B0D10` |
| `osum-wortmarke-mono.*` | einfarbig schwarz, Nadel mit Fuge (Gravur, Fax, Stempel) |
| `osum-wortmarke-mono-invers.*` | einfarbig weiss auf dunkel |
| `osum-signet-*.*` | nur das O mit Nadel — die abtrennbare Bildmarke |
| `osum-favicon-{16,32,48,64,128,256}.png`, `osum-favicon.ico` | Favicon-Satz |

Bei den einfarbigen Varianten ist die **Fuge (2 Einheiten) zwingend** — ohne sie
liest sich die Doppelnadel als Raute statt als Kompassnadel.

**Offen:** Der Zusatz `KERNEL` unter der Wortmarke ist bewusst NICHT in den SVGs.
In den bisherigen Entwuerfen war das DejaVu Sans, also nur ein Platzhalter.
Sobald die Hausschrift feststeht, kommt die Zeile als eigener Lockup dazu.
