# NETZWERK.md — OrientOS Netzwerk- und Anonymisierungsarchitektur

Status: Designdokument, festgehalten **bevor** der Netzwerkstack gebaut wird.
Grund: Die Umleitschicht muss von Anfang an mitgedacht werden — nachträglich
draufkleben (wie Linux mit iptables/TPROXY) erzeugt genau die Leaks, die wir
vermeiden wollen.

---

## 1. Grundprinzip

Der Kernel kennt **keine** Anonymisierungstechnik. Er kennt nur einen
**Umleit-Mechanismus** mit Toggle. Was hinter der Umleitung steckt (Tor,
WireGuard, eigenes Relay-Netz, direktes Kabel), ist reine Userspace-Konfiguration.

Trennlinie: **Datenpfad vs. Steuerung.**

| | Kernel (Ring 0) | Userspace |
|---|---|---|
| Rolle | klein, dumm, schnell | groß, schlau, austauschbar |
| Inhalt | Flag, Umleitung, fail-closed, Capability | Krypto, Protokolle, Pfadauswahl, Config |
| Umfang | ~500–1.000 Zeilen | beliebig |
| Absturz bedeutet | Panic (darum: minimal halten) | Dienst-Neustart, kein Datenverlust nach außen |

---

## 2. Kernel-Anteil (verbindlich klein)

### 2.1 Das Flag
```
netz_modus: AtomicU8
  0 = direkt
  1 = Tunnel      (Paketebene, z. B. WireGuard, TCP+UDP)
  2 = Strom-Proxy (Verbindungsebene, z. B. Tor via SOCKS5/TransPort, nur TCP+DNS)
```
Gelesen im Socket-Layer bei `connect()` / `sendto()` / DNS-Auflösung.

### 2.2 Die Umleitung
- Verbindungswunsch abfangen, Ziel merken, an **Handle X** (vom Userspace-Dienst
  registriert) weiterreichen.
- Der Kernel weiß **nicht**, was hinter Handle X steckt.

### 2.3 fail-closed (nicht verhandelbar)
- Modus ≠ 0 und Zieldienst nicht bereit ⇒ **Paket fällt**. Niemals Klartext.
- Muss im Kernel sitzen, sonst leakt es genau im Absturzmoment des Dienstes.
- Gilt auch beim Umschalten und beim Boot (Standard = fail-closed, bis der
  Dienst sich meldet).

### 2.4 Berechtigung
- Umschalten und Handle-Registrierung erfordern eine eigene **Capability**
  (`CAP_NETZ_UMLEITUNG`). Kein beliebiger Prozess darf das.

### 2.5 UDP-Regel
- Modus 1 (Tunnel): UDP erlaubt.
- Modus 2 (Strom-Proxy): UDP **gesperrt**, Ausnahme DNS über den Dienst.
  Sonst leaken QUIC/WebRTC die echte IP.

### 2.6 DNS
- DNS ist der klassische Leak. In Modus 1 und 2 geht Namensauflösung
  **zwingend** über den Dienst, nie am Umleiter vorbei.

---

## 3. Userspace-Anteil

Zuständig für alles Große:
- Tor-Client (zunächst: Anbindung an bestehenden Daemon per SOCKS5/TransPort)
- WireGuard-Handshake und Schlüsselverwaltung
- Verzeichnisse, Pfadauswahl, TLS
- Konfiguration, Profile, Serverlisten

### 3.1 Platzhalter statt fester Pfade

Namen und Pfade stehen **noch nicht fest**. In diesem Dokument (und im Code)
wird daher durchgehend mit Platzhaltern gearbeitet:

| Platzhalter | Bedeutung | Beispiel (unverbindlich) |
|---|---|---|
| `$KONFIG_WURZEL` | Wurzel aller Systemkonfiguration | `/etc/<os>` |
| `$NETZ_KONFIG` | Netzwerk-/Umleitkonfiguration | `$KONFIG_WURZEL/netz.toml` |
| `$SCHLUESSEL_WURZEL` | Ablage privater Schlüssel | `$KONFIG_WURZEL/keys` |

**Regel:** Kein fester Pfad im Code. Die Auflösung passiert an genau **einer**
Stelle (Pfad-Modul), alles andere fragt dort nach. Ändert sich der Name des
Systems oder das Layout, ist es ein Einzeiler.

Konfiguration `$NETZ_KONFIG`:
```toml
modus = 2            # 0 direkt | 1 tunnel | 2 proxy
fail_closed = true

[tunnel]
typ = "wireguard"
peer = "…"
schluessel = "$SCHLUESSEL_WURZEL/wg.priv"

[proxy]
typ = "socks5"
adresse = "127.0.0.1:9050"
dns = "127.0.0.1:5353"
```

Ablauf: Dienst liest Config → baut Verbindung auf → meldet dem Kernel
„Handle X ist bereit" → Kernel leitet um.

---

## 4. Schnittstellen in Firn

Eine Abstraktion, mehrere Umsetzungen. Der Kernel spricht nur die Schnittstelle an.

```
Tunnel        // Paketebene
  sende(paket)  -> Result
  empfange()    -> Paket
  // Umsetzungen: WireGuard, eigenes Netz

StromProxy    // Verbindungsebene
  verbinde(ziel) -> Strom
  // Umsetzungen: SOCKS5, Onion-Routing (Tor), direkt
```

Damit ist der Kernel-Teil generisch: heute Tor, morgen WireGuard, übermorgen
eigenes Relay-Netz — gleiche Anbindung, kein Umbau.

---

## 5. Reihenfolge (realistisch)

| Schritt | Inhalt | Aufwand | Anmerkung |
|---|---|---|---|
| 1 | **Netzwerkstack** (Ethernet, IP, TCP, UDP, DNS) | groß | OrientOS hat noch keinen. Ohne ihn ist alles Theorie. |
| 2 | **Umleitschicht + Toggle + fail-closed** | ~500–1.000 Z. | generisch, ohne konkrete Technik |
| 3 | **Schnittstellen `Tunnel` / `StromProxy` in Firn** | klein | verbaut nichts, definiert früh |
| 4 | **SOCKS5-Client in Firn** | ~300 Z. | Tor sofort nutzbar über bestehenden Daemon |
| 5 | **WireGuard in Firn** | ~4.000 Z. | erster echter `secret[T]`-Vorzeigefall (Curve25519, ChaCha20-Poly1305, BLAKE2s) |
| 6 | **TLS-1.3-Client in Firn** | groß | wird für den Browser ohnehin gebraucht — Doppelnutzen |
| 7 | **Eigener Tor-Client in Firn** | ~30–100k Z. | erst nach Schritt 6 realistisch. Vorbild: Arti |

---

## 6. Bewusste Entscheidungen und ihre Begründung

**Kein eigenes Relay-Netz zum Start.**
Ein Netz mit 5 Nutzern anonymisiert niemanden. Tor hat ~8.000 Relays, Millionen
Nutzer, 20 Jahre Kryptoanalyse. Eigenes Protokoll wäre am ersten Tag kaputt —
man würde es nur nicht merken. Exit-Node-Haftung entfällt zusätzlich.

**Krypto gehört nicht in Ring 0.**
Protokoll-Parser sind die Bug-Quelle Nummer eins. Ein Bug im Relay-Protokoll
wäre im Kernel sofort Totalkompromiss — absurd bei einem OS, das auf
Capabilities setzt.

**Ausnahme WireGuard, mit Auflage.**
Linux hat WireGuard im Kernel (Durchsatz: sonst zweimal kopieren pro Paket).
Für OrientOS zunächst trotzdem Userspace. Falls Messungen später Handlungsbedarf
zeigen: **nur der Datenpfad** (ChaCha20 auf dem Paket) wandert in den Kernel,
der **Handshake bleibt draußen**.

**VPN ≠ Tor.**

| | WireGuard/VPN | Tor |
|---|---|---|
| Hops | 1 | 3 |
| Sieht IP + Ziel | der Anbieter (Vertrauen nötig) | niemand |
| Tempo | ~Volltempo, ~10 ms | 1–3 Mbit/s, 200–500 ms |
| UDP | ja | nein |
| Code | ~4.000 Zeilen | ~200k Zeilen C (bzw. Arti ~100k Rust) |

VPN verschiebt Vertrauen, Tor liefert Anonymität. Deshalb beide Modi, nicht einer.

**Tor-Netzwerk nutzen ≠ Tor-Code nutzen.**
`tor-spec.txt` ist öffentlich. Ein eigener Client darf sich mit den echten
Relays verbinden (Präzedenzfall: Arti). Damit fällt die C-Abhängigkeit
langfristig weg — aber erst nach dem TLS-Client.

**Gefahrenhinweis für Schritt 7:**
Fehler in der Pfadauswahl brechen die Anonymität, **ohne dass etwas kaputt
aussieht**. Es funktioniert scheinbar — man ist nur deanonymisiert. Dieser
Schritt braucht externe Prüfung, nicht nur Tests.

---

## 7. Prüfkriterien (für spätere Gauntlet-Runden)

- [ ] Modus ≠ 0 + Dienst tot ⇒ **kein einziges Paket** verlässt das System (Test mit Paketmitschnitt)
- [ ] Kein DNS am Umleiter vorbei (auch nicht beim Boot, auch nicht bei Timeout)
- [ ] Modus 2 + UDP-Sendeversuch ⇒ abgewiesen, nicht durchgereicht
- [ ] Umschalten ohne Capability ⇒ abgewiesen
- [ ] Kernel-Umleitcode messbar < 1.000 Zeilen, ohne Krypto, ohne Parser
- [ ] Dienst-Neustart ohne Reboot, ohne Leak im Übergangsfenster
- [ ] Kein fest verdrahteter Konfigpfad im Code — alles über das Pfad-Modul
