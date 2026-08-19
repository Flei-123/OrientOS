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
