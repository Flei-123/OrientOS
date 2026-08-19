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

## Stand: FESTGELEGT (Runde 5)

Grundlage ist Justins Vorlage `osum-vorlage-justin.jpg`. Die wurde nicht
nachgebaut, sondern mit **potrace vektorisiert** — die Konturen sind also
exakt die der Vorlage, nur als saubere Bezierkurven statt als JPEG.

`osum-master.json` = der Master mit drei Ebenen:

| Ebene | Inhalt |
|---|---|
| `letters` | OSUM + KERNEL |
| `n_hell` | helle Facette der Kompassnadel (oben rechts) |
| `n_dunkel` | dunkle Facette der Kompassnadel (unten links) |

Normiert auf **Versalhoehe OSUM = 100 Einheiten**. Lockup misst 370,6 x 148,8 Einheiten.

### Farben

| Rolle | Wert |
|---|---|
| Schrift / Ring | `#0B0D10` |
| Nadel hell | `#05EEFA` |
| Nadel dunkel | `#0A8098` |
| Heller Grund | `#FFFFFF` |

### Bauen

    python3 logo_build.py

Braucht `cairosvg` und `Pillow` (beides auf dem Server vorhanden). Erzeugt je
Modus Lockup, Wortmarke und Signet als SVG+PNG, dazu den Favicon-Satz.
Nichts wird von Hand nachgezogen — Aenderungen immer ueber `logo_build.py`.

### Dateien

| Datei | Zweck |
|---|---|
| `osum-lockup-*.{svg,png}` | vollstaendige Marke: OSUM mit KERNEL darunter |
| `osum-wortmarke-*.{svg,png}` | nur OSUM, ohne die Zeile KERNEL |
| `osum-signet-*.{svg,png}` | nur das O mit Nadel — die abtrennbare Bildmarke |
| `osum-favicon-{16,32,48,64,128,256}.png`, `osum-favicon.ico` | Favicon-Satz |

Modi: `hell`, `dunkel`, `mono`, `mono-invers`.

### Regeln

* Bei den **einfarbigen** Varianten wird automatisch eine **Fuge** zwischen die
  beiden Nadelfacetten gesetzt. Ohne sie verschmelzen sie zu einer formlosen
  Raute und die Nadel ist nicht mehr als Nadel lesbar.
* Das Signet ist die Kleinform und funktioniert bis **16 px** herunter.
  Fuer alles unter ~120 px Breite Signet statt Lockup nehmen, sonst ist
  `KERNEL` nur noch Grau.
* Der Ring des O darf nie ohne Nadel stehen — sonst ist es nur ein Kreis.

### Historie

Verworfen wurden vorher: Punkt-im-O, Ring-im-Ring, Tor (las sich klein als `C`,
das Wort wurde zu `CSUM`), einfarbige Nadel ohne Fuge, sowie eine komplett
selbst konstruierte Buchstabengeometrie (Strichstaerke 26 zu fett; deren
S-Pfad hatte ausserdem am zweiten Bogen `sweep-flag 0` statt `1` und war
dadurch ein ueberlappender Klecks).
