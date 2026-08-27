# NETZWERK.md — OrientOS network and anonymisation architecture

Status: a design document, written down **before** the network stack is built.
Reason: the redirection layer has to be thought through from the start —
gluing it on afterwards (the way Linux does with iptables/TPROXY) produces
exactly the leaks we want to avoid.

---

## 1. Basic principle

The kernel knows **no** anonymisation technique. It knows only a
**redirection mechanism** with a toggle. What sits behind the redirection
(Tor, WireGuard, a relay network of our own, a direct cable) is pure userspace
configuration.

The dividing line: **data path vs. control.**

| | kernel (ring 0) | userspace |
|---|---|---|
| role | small, dumb, fast | large, smart, exchangeable |
| content | flag, redirection, fail-closed, capability | crypto, protocols, path selection, config |
| size | ~500–1,000 lines | any |
| a crash means | panic (hence: keep it minimal) | a service restart, no data loss to the outside |

---

## 2. The kernel's share (bindingly small)

### 2.1 The flag
```
netz_modus: AtomicU8
  0 = direkt      (direct)
  1 = Tunnel      (packet level, e.g. WireGuard, TCP+UDP)
  2 = Strom-Proxy (connection level, e.g. Tor via SOCKS5/TransPort, TCP+DNS only)
```
Read in the socket layer at `connect()` / `sendto()` / name resolution.

### 2.2 The redirection
- Intercept the connection request, remember the target, hand it on to
  **handle X** (registered by the userspace service).
- The kernel does **not** know what sits behind handle X.

### 2.3 fail-closed (not negotiable)
- Mode ≠ 0 and the target service not ready ⇒ **the packet is dropped**. Never
  cleartext.
- It has to sit in the kernel, otherwise it leaks at exactly the moment the
  service crashes.
- It also applies while switching and at boot (default = fail-closed until the
  service reports in).

### 2.4 Authorisation
- Switching and registering a handle require a **capability** of their own
  (`CAP_NETZ_UMLEITUNG`). Not just any process may do it.

### 2.5 UDP rule
- Mode 1 (tunnel): UDP allowed.
- Mode 2 (stream proxy): UDP **blocked**, with DNS through the service as the
  exception. Otherwise QUIC/WebRTC leak the real IP.

### 2.6 DNS
- DNS is the classic leak. In modes 1 and 2 name resolution goes **compulsorily**
  through the service, never past the redirector.

---

## 3. The userspace share

Responsible for everything large:
- Tor client (at first: attaching to an existing daemon over SOCKS5/TransPort)
- WireGuard handshake and key management
- directories, path selection, TLS
- configuration, profiles, server lists

### 3.1 Placeholders instead of fixed paths

Names and paths are **not settled yet**. In this document (and in the code)
placeholders are therefore used throughout:

| placeholder | meaning | example (not binding) |
|---|---|---|
| `$KONFIG_WURZEL` | the root of all system configuration | `/etc/<os>` |
| `$NETZ_KONFIG` | network/redirection configuration | `$KONFIG_WURZEL/netz.toml` |
| `$SCHLUESSEL_WURZEL` | where private keys are kept | `$KONFIG_WURZEL/keys` |

**Rule:** no fixed path in the code. The resolution happens in exactly **one**
place (the path module), everything else asks there. If the name of the system
or the layout changes, it is a one-liner.

Configuration `$NETZ_KONFIG`:
```toml
modus = 2            # 0 direct | 1 tunnel | 2 proxy
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

The sequence: the service reads the config → establishes the connection →
reports "handle X is ready" to the kernel → the kernel redirects.

---

## 4. Interfaces in Firn

One abstraction, several implementations. The kernel only addresses the
interface.

```
Tunnel        // packet level
  sende(paket)  -> Result
  empfange()    -> Paket
  // implementations: WireGuard, own network

StromProxy    // connection level
  verbinde(ziel) -> Strom
  // implementations: SOCKS5, onion routing (Tor), direct
```

That makes the kernel part generic: Tor today, WireGuard tomorrow, a relay
network of our own the day after — the same attachment, no rebuild.

---

## 5. Order (realistic)

| step | content | effort | remark |
|---|---|---|---|
| 1 | **Network stack** (Ethernet, IP, TCP, UDP, DNS) | large | OrientOS has none yet. Without it everything is theory. |
| 2 | **Redirection layer + toggle + fail-closed** | ~500–1,000 l. | generic, without a concrete technique |
| 3 | **The interfaces `Tunnel` / `StromProxy` in Firn** | small | blocks nothing, defines early |
| 4 | **SOCKS5 client in Firn** | ~300 l. | Tor usable immediately through an existing daemon |
| 5 | **WireGuard in Firn** | ~4,000 l. | the first real showcase for `secret[T]` (Curve25519, ChaCha20-Poly1305, BLAKE2s) |
| 6 | **TLS 1.3 client in Firn** | large | needed for the browser anyway — double benefit |
| 7 | **A Tor client of our own in Firn** | ~30–100k l. | only realistic after step 6. Model: Arti |

---

## 6. Deliberate decisions and their reasoning

**No relay network of our own to start with.**
A network with 5 users anonymises nobody. Tor has ~8,000 relays, millions of
users, 20 years of cryptanalysis. A protocol of our own would be broken on day
one — you just would not notice. Exit node liability drops away as well.

**Crypto does not belong in ring 0.**
Protocol parsers are bug source number one. A bug in the relay protocol would
be an immediate total compromise inside the kernel — absurd for an OS that
builds on capabilities.

**WireGuard as the exception, with a condition.**
Linux has WireGuard in the kernel (throughput: otherwise it is two copies per
packet). For OrientOS it stays in userspace at first. If measurements later
show a need to act: **only the data path** (ChaCha20 on the packet) moves into
the kernel, the **handshake stays outside**.

**VPN ≠ Tor.**

| | WireGuard/VPN | Tor |
|---|---|---|
| hops | 1 | 3 |
| sees IP + target | the provider (trust required) | nobody |
| speed | ~full speed, ~10 ms | 1–3 Mbit/s, 200–500 ms |
| UDP | yes | no |
| code | ~4,000 lines | ~200k lines of C (or Arti ~100k of Rust) |

A VPN shifts trust, Tor delivers anonymity. That is why both modes exist, not
one.

**Using the Tor network ≠ using Tor's code.**
`tor-spec.txt` is public. A client of our own may connect to the real relays
(precedent: Arti). That way the C dependency drops away in the long run — but
only after the TLS client.

**A warning for step 7:**
Mistakes in the path selection break anonymity **without anything looking
broken**. It seems to work — you are just deanonymised. This step needs
external review, not just tests.

---

## 7. Acceptance criteria (for later gauntlet rounds)

- [ ] Mode ≠ 0 + the service dead ⇒ **not a single packet** leaves the system (test with a packet capture)
- [ ] No DNS past the redirector (not at boot either, not on a timeout either)
- [ ] Mode 2 + an attempt to send UDP ⇒ refused, not passed through
- [ ] Switching without the capability ⇒ refused
- [ ] The kernel redirection code measurably < 1,000 lines, without crypto, without a parser
- [ ] A service restart without a reboot, without a leak in the transition window
- [ ] No hard-wired config path in the code — everything through the path module
