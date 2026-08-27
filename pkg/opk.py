#!/usr/bin/env python3
"""pkg/opk.py -- die Paketverwaltung von OrientOS.

Der Entwurf steht in PACKAGING.md und ist aelter als dieses Werkzeug; das
Format, das hier wirklich gebaut wird, steht in PAKETE.md. Umgesetzt sind
die vier Regeln aus PACKAGING.md § 1, und zwar so, dass sie sich MESSEN
lassen:

  1. Ein Paket wird zu EINEM unveraenderlichen Verzeichnis unter
     `store/<hash>`, benannt nach der SHA-256 seines Inhalts.
  2. Nutzerdaten liegen in drei Toepfen: `users/<wer>/config|state|cache`.
  3. Entfernen heisst: die aktivierte Sicht und die Nutzerdaten loeschen.
     Der Store-Eintrag bleibt, solange eine GENERATION ihn nennt -- das
     ist keine Nachlaessigkeit, sondern die Bedingung dafuer, dass man
     zurueckrollen kann.
  4. Es gibt keine Registry. Der ganze Zustand des Systems ist die Datei
     `system/generations/<n>/PLAN`, und die ist Text.

DREI ENTSCHEIDUNGEN, DIE MAN SEHEN MUSS, WEIL SIE DEN REST ERKLAEREN.

*Keine Zeitstempel, nirgends.* Weder im Paket noch in einer Generation.
Ein Zeitstempel macht zwei gleiche Vorgaenge unterscheidbar, und dann
laesst sich „nach Installieren und Entfernen ist alles wie vorher" nicht
mehr messen, sondern nur noch behaupten. Wer wissen will, wann etwas
geschah, liest die Historie des Repos.

*Harte Verweise statt Kopien.* `apps/<name>.prog/start` ist ein ZWEITER
NAME auf dieselbe Datei im Store, kein zweites Exemplar -- dasselbe
Verfahren, mit dem Osums Runde K15 `/bin/explorer` und `/bin/files`
verbindet, und dasselbe, das `mkfs.py` mit `<neu>@<vorhanden>` in ein
OFS-Abbild schreibt. Auf dem Wirt ist es `os.link`, und dass es wirklich
derselbe Inode ist, wird gemessen (`st_ino`) und nicht angenommen.

*Kein Schreiben nach `bin/`.* PACKAGING.md § 6 nennt die
Kompatibilitaetsverweise nach `/usr/bin` ausdruecklich das, was man NICHT
uebernimmt -- „die Hintertuer, die alles wieder aufweicht". Ein Paket
legt deshalb nichts unter `bin/` ab. Was dort liegt, kommt aus
`userland/PROGRAMME` und ist das alte, ungepackte Userland.

Die Befehle:

    opk.py bauen <rezept> -o <datei.opk>
    opk.py zeigen <datei.opk>
    opk.py installieren --wurzel W <datei.opk | name>
    opk.py entfernen    --wurzel W <name>
    opk.py liste        --wurzel W
    opk.py aktualisieren --wurzel W [<name>]
    opk.py generationen --wurzel W
    opk.py zurueck      --wurzel W <n>
    opk.py aufraeumen   --wurzel W [--behalte N]
    opk.py pruefen      --wurzel W
    opk.py baum         --wurzel W [--ohne system]
    opk.py verweise     --wurzel W
    opk.py schluessel   <verzeichnis>
    opk.py quelle       <verzeichnis> [--schluessel <datei>]

Round PLAN2 added the typed PLAN and, with it, these (English) verbs.
`--root` is the same flag as `--wurzel`:

    opk.py plan          --root R
    opk.py export        --root R -o <datei>
    opk.py kernel        --root R <datei.opk | name | none>
    opk.py source-add    --root R <url> <pubkey-hex | datei>
    opk.py source-remove --root R <url>
    opk.py set           --root R <key> <value>
    opk.py unset         --root R <key>
    opk.py settings      --root R
    opk.py account-add   --root R <name> [--uid --gid --home --shell]
    opk.py account-remove --root R <name>
    opk.py secret-set    --root R <name> <datei | ->
    opk.py snapshot      --root R
    opk.py verify        --root R
    opk.py rebuild       --root R --plan <datei> [--source D] [--secrets D]
"""

import argparse
import hashlib
import os
import shutil
import struct
import sys

KENNUNG = b"OPKG0001"
KOPF = 64                       # Oktette Kopf, siehe PAKETE.md

# WIE LANG DER NAME EINES STORE-EINTRAGS IST, und warum nicht 64.
#
# Ein Verzeichniseintrag in OFS ist 32 Oktette: acht fuer die
# Inode-Nummer, vierundzwanzig fuer den Namen, davon einer fuer die Null
# (`kernel/fs.fi`, `NAME_LEN = 24`). Ein SHA-256 in Hexziffern ist 64
# Zeichen lang und passt da nicht hinein -- der erste Versuch, diese
# Wurzel in ein Abbild zu schreiben, endete woertlich mit
# `mkfs: the name '478632825585...' is too long`.
#
# Der Ausweg ist NICHT, den Hash zu ersetzen: die IDENTITAET eines
# Pakets bleibt die volle SHA-256, sie steht in jedem PLAN und wird bei
# jeder Installation und bei `pruefen` ueber den ganzen Inhalt
# nachgerechnet. Verkuerzt ist nur der NAME des Verzeichnisses, auf
# zwanzig Hexziffern = 80 Bit. Das ist derselbe Handel, den Nix mit
# seinen 32 Base-32-Zeichen macht, nur an eine kleinere Zahl angepasst.
#
# Und die Kollision wird nicht weggehofft: `store_schreiben` liest bei
# einem vorhandenen Verzeichnis dessen PAKET, rechnet den vollen Hash
# nach und BRICHT AB, wenn er ein anderer ist. Zwei verschiedene Pakete
# mit denselben achtzig Bit waeren dann ein lauter Fehler und kein
# stilles Ueberschreiben.
KURZ = 20


def kurz(h):
    """Der Verzeichnisname im Store: die ersten KURZ Hexziffern."""
    return h[:KURZ]

# ---------------------------------------------------------------------------
# Ed25519, ZWEIMAL.
#
# Signaturen (Roadmap 6.2) sind der Punkt, an dem eine einzige Umsetzung
# am wenigsten wert ist: eine Pruefung, die IMMER „ja" sagt, faellt bei
# keinem gueltigen Paket auf. Deshalb steht hier eine eigene, kurze
# Umsetzung von Ed25519 (RFC 8032) NEBEN der Bibliothek des Wirts, und
# `quelle` wie `installieren` rechnen beide und vergleichen. Derselbe
# Gedanke wie bei `mkfs.py`, das die zweite Umsetzung des Dateisystems
# ist: zwei Programme koennen sich irren, aber selten gleich.
# ---------------------------------------------------------------------------
_P = 2 ** 255 - 19
_L = 2 ** 252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_I = pow(2, (_P - 1) // 4, _P)


def _sha512(b):
    return hashlib.sha512(b).digest()


def _xrecover(y):
    xx = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P)
    x = pow(xx, (_P + 3) // 8, _P)
    if (x * x - xx) % _P != 0:
        x = (x * _I) % _P
    if x % 2 != 0:
        x = _P - x
    return x


_BY = (4 * pow(5, _P - 2, _P)) % _P
_B = [_xrecover(_BY) % _P, _BY]


def _edwards(p, q):
    x1, y1 = p
    x2, y2 = q
    k = _D * x1 * x2 * y1 * y2
    x3 = (x1 * y2 + x2 * y1) * pow(1 + k, _P - 2, _P)
    y3 = (y1 * y2 + x1 * x2) * pow(1 - k, _P - 2, _P)
    return [x3 % _P, y3 % _P]


def _scalarmult(p, e):
    if e == 0:
        return [0, 1]
    q = _scalarmult(p, e // 2)
    q = _edwards(q, q)
    if e & 1:
        q = _edwards(q, p)
    return q


def _encodepoint(p):
    x, y = p
    bits = [(y >> i) & 1 for i in range(255)] + [x & 1]
    return bytes(sum(bits[i * 8 + j] << j for j in range(8)) for i in range(32))


def _bit(h, i):
    return (h[i // 8] >> (i % 8)) & 1


def ed25519_public(sk):
    h = _sha512(sk)
    a = 2 ** 254 + sum(2 ** i * _bit(h, i) for i in range(3, 254))
    return _encodepoint(_scalarmult(_B, a))


def ed25519_sign(sk, msg):
    h = _sha512(sk)
    a = 2 ** 254 + sum(2 ** i * _bit(h, i) for i in range(3, 254))
    pk = _encodepoint(_scalarmult(_B, a))
    r = int.from_bytes(_sha512(h[32:64] + msg), "little") % _L
    rr = _scalarmult(_B, r)
    er = _encodepoint(rr)
    k = int.from_bytes(_sha512(er + pk + msg), "little") % _L
    s = (r + k * a) % _L
    return er + s.to_bytes(32, "little")


def _decodepoint(s):
    y = int.from_bytes(s, "little") & ((1 << 255) - 1)
    x = _xrecover(y)
    if x & 1 != _bit(s, 255):
        x = _P - x
    p = [x, y]
    if (-p[0] * p[0] + p[1] * p[1] - 1 - _D * p[0] * p[0] * p[1] * p[1]) % _P != 0:
        raise ValueError("kein Punkt auf der Kurve")
    return p


def ed25519_verify(pk, msg, sig):
    if len(sig) != 64 or len(pk) != 32:
        return False
    try:
        rr = _decodepoint(sig[:32])
        a = _decodepoint(pk)
    except Exception:
        return False
    s = int.from_bytes(sig[32:64], "little")
    k = int.from_bytes(_sha512(sig[:32] + pk + msg), "little") % _L
    links = _scalarmult(_B, s)
    rechts = _edwards(rr, _scalarmult(a, k))
    return _encodepoint(links) == _encodepoint(rechts)


def ed25519_verify_bibliothek(pk, msg, sig):
    """Die zweite Meinung. Gibt None, wenn der Wirt keine Bibliothek hat."""
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 \
            import Ed25519PublicKey
        from cryptography.exceptions import InvalidSignature
    except Exception:
        return None
    try:
        Ed25519PublicKey.from_public_bytes(pk).verify(sig, msg)
        return True
    except InvalidSignature:
        return False
    except Exception:
        return False


# ---------------------------------------------------------------------------
# Das Paketformat. PAKETE.md § 2 beschreibt es in Worten.
# ---------------------------------------------------------------------------
FELDER = ("name", "fassung", "titel", "info", "keys")   # feste Reihenfolge


def meta_bauen(felder, braucht, handles):
    zeilen = []
    for f in FELDER:
        zeilen.append("%s=%s" % (f, felder.get(f, "")))
    for b in sorted(braucht):
        zeilen.append("braucht=%s" % b)
    for h in sorted(handles):
        zeilen.append("handle=%s" % h)
    # EXTRA FIELDS, added in round PLAN2 for `kind=kernel` and `origin=`.
    # They come LAST and sorted, so a recipe without any of them produces
    # exactly the octets it produced before -- no package that already
    # exists changes its hash because this line was added.
    for k in sorted(felder):
        if k in FELDER or k in ("braucht", "handle"):
            continue
        zeilen.append("%s=%s" % (k, felder[k]))
    return ("\n".join(zeilen) + "\n").encode("utf-8")


def meta_lesen(roh):
    felder, braucht, handles = {}, [], []
    for zeile in roh.decode("utf-8").splitlines():
        if not zeile or zeile.startswith("#"):
            continue
        k, _, v = zeile.partition("=")
        if k == "braucht":
            braucht.append(v)
        elif k == "handle":
            handles.append(v)
        else:
            felder[k] = v
    return felder, braucht, handles


def archiv_bauen(wurzel):
    """Ein DETERMINISTISCHES Archiv eines Verzeichnisses.

    Kein tar: tar traegt Zeit, Eigentuemer und Fuellung mit sich, und
    zwei Laeufe ueber denselben Baum ergeben verschiedene Oktette. Dann
    ist „gleicher Hash = gleicher Inhalt" nicht mehr wahr, und der ganze
    Entwurf haengt daran. Sortiert wird ueber die OKTETTE des Namens.
    """
    eintraege = []
    for pfad, verz, dateien in os.walk(wurzel):
        rel = os.path.relpath(pfad, wurzel)
        verz.sort()
        if rel != ".":
            eintraege.append(("d", rel.replace(os.sep, "/"), None))
        for d in sorted(dateien):
            p = os.path.join(pfad, d)
            r = os.path.relpath(p, wurzel).replace(os.sep, "/")
            eintraege.append(("f", r, p))
    eintraege.sort(key=lambda e: e[1].encode("utf-8"))
    aus = bytearray()
    for typ, name, quelle in eintraege:
        nb = name.encode("utf-8")
        if typ == "d":
            modus, daten = 0o755, b""
        else:
            daten = open(quelle, "rb").read()
            # Der Modus wird NICHT vom Wirt uebernommen: dessen umask
            # haette sonst Einfluss auf den Hash. Ausfuehrbar ist, was
            # `start` heisst -- alles andere ist Datei.
            modus = 0o755 if name.split("/")[-1] == "start" else 0o644
        aus += typ.encode("ascii")
        aus += struct.pack("<HH", modus, len(nb))
        aus += nb
        aus += struct.pack("<Q", len(daten))
        aus += daten
    return bytes(aus)


def archiv_lesen(roh):
    i, aus = 0, []
    while i < len(roh):
        typ = chr(roh[i]); i += 1
        modus, nl = struct.unpack_from("<HH", roh, i); i += 4
        name = roh[i:i + nl].decode("utf-8"); i += nl
        (dl,) = struct.unpack_from("<Q", roh, i); i += 8
        daten = roh[i:i + dl]; i += dl
        aus.append((typ, name, modus, daten))
    return aus


def paket_packen(meta, daten):
    inhalt = hashlib.sha256(meta + daten).digest()
    kopf = bytearray(KOPF)
    kopf[0:8] = KENNUNG
    struct.pack_into("<QQ", kopf, 8, len(meta), len(daten))
    kopf[24:56] = inhalt
    return bytes(kopf) + meta + daten, inhalt.hex()


def paket_lesen(roh):
    """Gibt (meta, daten, hash) zurueck -- oder wirft.

    DIE PRUEFUNG IST HIER UND NUR HIER. Jeder Weg ins System geht durch
    diese Funktion, damit es keinen zweiten gibt, der sie vergisst.
    """
    if len(roh) < KOPF or roh[0:8] != KENNUNG:
        raise ValueError("keine OPKG-Datei (Kennung fehlt)")
    ml, dl = struct.unpack_from("<QQ", roh, 8)
    genannt = roh[24:56]
    if KOPF + ml + dl != len(roh):
        raise ValueError("Laengen im Kopf passen nicht zur Datei "
                         "(%d + %d + %d != %d)" % (KOPF, ml, dl, len(roh)))
    meta = roh[KOPF:KOPF + ml]
    daten = roh[KOPF + ml:KOPF + ml + dl]
    gerechnet = hashlib.sha256(meta + daten).digest()
    if gerechnet != genannt:
        raise ValueError("Pruefsumme falsch: im Kopf steht %s, gerechnet %s"
                         % (genannt.hex()[:16], gerechnet.hex()[:16]))
    return meta, daten, gerechnet.hex()


# ---------------------------------------------------------------------------
# THE PLAN, VERSION 2 -- TYPED LINES
#
# Everything from here on is written in English (round PLAN2). The older
# German code above is untouched on purpose: renaming the existing body is
# a round of its own, and mixing a rename into a format change would make
# both unreviewable.
#
# WHY THE PLAN HAD TO GROW. Until this round a PLAN was `name<TAB>sha256`
# and nothing else, so it described the installed APPLICATIONS and only
# those. A machine cannot be rebuilt from that: it does not say which
# kernel the applications run on, it does not say WHERE the hashes can be
# fetched from, and it does not say what the machine is configured to be
# (time, network, accounts). Three quarters of a system state were outside
# the one file that claims to be the system state.
#
# THE SHAPE. A line is TAB separated. The first field is a TYPE:
#
#     app       <name>   <sha256>            an installed application
#     kernel    <sha256>                     the kernel this generation runs
#     source    <url>    <ed25519-pubkey>    where hashes may be fetched
#     setting   <key>    <value>             one system setting
#     account   <name> <uid> <gid> <home> <shell> <secret>
#
# READING AN OLD PLAN. The rule is one line long and needs no version
# header, no migration and no rewrite of files that already exist:
#
#     fields = line.split(TAB)
#     if fields[0] in PLAN_TYPES  -> typed line
#     elif len(fields) == 2       -> the old `name<TAB>hash`, i.e. an app
#     else                        -> error, and say which line
#
# The one thing that rule can get wrong is an application literally named
# `app`, `kernel`, `source`, `setting` or `account` -- five names. They are
# therefore REFUSED at build time and at install time (`reserved_name`),
# which turns a silent misreading into a loud error at the only moment
# where it can still be fixed.
#
# THE FILE IS WHAT `sort` PRODUCES. Lines are written sorted by their
# bytes -- the whole line, not by type and then by name. That is not a
# detail: it means the canonical order can be checked from the outside
# with `LC_ALL=C sort -c PLAN` and needs no knowledge of this program.
# Alphabetically the types fall into the order account, app, kernel,
# setting, source, which reads well enough by accident.
#
# WHAT IS NOT IN THE PLAN, AND WHY: PASSWORDS. A PLAN is meant to be
# handed to someone else -- that is the whole point of `rebuild`. A
# password hash in it would be a password hash in every backup, every
# paste and every repository that ever saw the file. The account line
# therefore carries the SHA-256 OF THE CREDENTIAL, not the credential:
#
#     account justin 1000 1000 /users/justin /bin/sh 6f1c...  (or `-`)
#
# The credential itself lives in `system/secrets/<name>`, mode 0600, and
# is never part of a PLAN. Two things follow, and both are wanted:
# changing a password IS a new generation (the hash in the plan changes,
# so it can be rolled back), and a rebuild from a PLAN ALONE produces the
# accounts with the passwords LOCKED. That is the honest outcome and it is
# measured as such -- see `docs/ROUND-PLAN2.md`.
# ---------------------------------------------------------------------------

PLAN_TYPES = ("account", "app", "kernel", "pref", "setting", "source")

# ---------------------------------------------------------------------------
# TWO LEVELS, AND WHY THEY ARE TWO (round PLAN2, addendum of 27.08.2026)
#
# The question that started this: "if I set up a new device from the plan,
# does it LOOK the same?" Until this addendum the answer was no, and for
# two separate reasons.
#
# THE FIRST REASON was that appearance was not in the system state at all.
# Osum's settings application (`kernel/user/einstellungen.fi`, 788 lines)
# writes /etc/theme, /etc/schemas/, /etc/hintergrund, /etc/schirm.conf,
# /etc/zeit.conf and /etc/netz.conf, and NONE of it was in a PLAN. A
# rebuilt machine came up with the applications of the old one and the
# looks of a fresh install.
#
# THE SECOND REASON is subtler and is the one worth getting right: half of
# those files are at the WRONG LEVEL.
#
#     SYSTEM (one per machine, needs privilege, `setting` -> /etc/)
#         screen resolution, network, timezone, hostname, keymap, accounts
#
#     USER   (one per person, `pref` -> /users/<who>/config/)
#         colour scheme, wallpaper, taskbar edge/height/autohide
#
# The test is not "is it about looks", it is: WOULD TWO PEOPLE ON ONE
# MACHINE WANT DIFFERENT ANSWERS? Two people share one screen resolution
# and one network -- they cannot each have their own. Two people do not
# share a wallpaper. `/etc/theme` is therefore not a small mistake, it is
# a place where a second user cannot exist.
#
# The consequence is honest and stated where it hurts: Osum's taskbar and
# desktop read the absolute paths /etc/theme and /etc/hintergrund today.
# Moving the truth to the user does not move those programs. So activation
# also writes a COMPATIBILITY VIEW under /etc -- hard links to the very
# same store octets -- for the single account of a single-user machine,
# and writes NO such view when there are two accounts, because then there
# is no honest answer to "whose theme is /etc/theme". That is not a
# limitation of this program; it is the bug being visible.
# ---------------------------------------------------------------------------

# WHAT A PLAN LINE HOLDS AND WHAT A FILE HOLDS.
#
#     A PLAN LINE HOLDS A DECISION. A FILE HOLDS CONTENT.
#
# `taskbar.edge=left` is a decision: four possible values, meaningless to
# store twice, and it belongs in the text. A wallpaper is content -- an
# image, and no image goes into a tab-separated line. Content is therefore
# CONTENT-ADDRESSED: the octets become an asset package in the store like
# any other package, and the plan carries the SHA-256.
#
# The alternative that was rejected: a second, non-package blob store
# (`blobs/<hash>`). It would have meant a second collector, a second
# verifier, a second fetch path and a second thing to sign -- all for
# something `store/` already does. An asset package is an `.opk` with
# `kind=asset`, one file called `blob`, and it travels through the signed
# INDEX exactly like a program does.
USER_BLOB_KEYS = ("theme", "wallpaper")

USER_KEYS = {
    # key                 file it renders                       kind
    "theme":            ("config/desktop/theme",            "blob"),
    "wallpaper":        ("config/desktop/wallpaper",        "blob"),
    "taskbar.edge":     ("config/desktop/taskbar.conf",     "scalar"),
    "taskbar.height":   ("config/desktop/taskbar.conf",     "scalar"),
    "taskbar.autohide": ("config/desktop/taskbar.conf",     "scalar"),
}

# Everything activation generates BELOW a user directory. Same discipline
# as GENERATED_FILES: a fixed, named list, deleted and rewritten on every
# activation, so that a preference which disappears takes its file with
# it. Nothing else under `config/` is ever touched -- the user's own files
# live there too, and they are not ours.
USER_GENERATED = ("config/desktop/taskbar.conf", "config/desktop/theme",
                  "config/desktop/wallpaper")

# THE COMPATIBILITY VIEW. Osum reads these three absolute paths today
# (`kernel/user/leiste.fi` line 579, `kernel/user/schreibtisch.fi` lines
# 203-204, `kernel/user/einstellungen.fi` line 489). They are SECOND NAMES
# on the user's files, not copies, and they exist only while a machine has
# exactly one account. When round DESKTOP teaches those programs to read
# `/users/<who>/config/desktop/`, this tuple becomes empty and nothing
# else changes.
COMPAT_FILES = ("etc/hintergrund", "etc/taskbar.conf", "etc/theme")
SCHEMA_DIR = "etc/schemas"

TASKBAR_EDGES = ("bottom", "left", "right", "top")   # wlibc.fi e_names
TASKBAR_DEFAULTS = {"taskbar.edge": "bottom",
                    "taskbar.height": "28",          # leiste.fi, `const PH`
                    "taskbar.autohide": "no"}

# THE CLOSED SET OF SETTINGS. An open key space would turn the PLAN into
# the junk drawer that PACKAGING.md § 1.4 exists to prevent: anything
# could be written anywhere and nothing could be rebuilt from the file.
# A key that is not in this table is refused by `set`. Adding one is a
# change to this program -- which is exactly the amount of friction it
# should have.
#
# The third column is the honest part: `consumer` says whether anything in
# the running system READS the file that this key produces. Osum reads
# `/etc/zeit.conf` (kernel/user/einstellungen.fi), `/etc/schirm.conf` and
# `/etc/netz.conf` (also kernel/user/dhcp.fi). Nothing in Osum reads a
# hostname or a keymap today -- those keys are recorded and rendered, and
# that is all they do until someone writes the consumer.
SETTING_KEYS = {
    # key             file                consumer in the running system
    "hostname":      ("etc/hostname",     None),
    "timezone":      ("etc/timezone",     None),
    "keymap":        ("etc/keymap",       None),
    "time.offset":   ("etc/zeit.conf",    "kernel/user/einstellungen.fi"),
    # `display.mode` and not `screen.mode`: the key is ours and one day
    # old, so renaming it costs nothing and English is the rule. The FILE
    # keeps Osum's name `schirm.conf` -- renaming another project's file
    # is round RENAME's job, not ours.
    "display.mode":  ("etc/schirm.conf",  "kernel/user/einstellungen.fi"),
    "net.mode":      ("etc/netz.conf",    "kernel/user/dhcp.fi"),
    "net.address":   ("etc/netz.conf",    "kernel/user/dhcp.fi"),
    "net.netmask":   ("etc/netz.conf",    "kernel/user/dhcp.fi"),
    "net.gateway":   ("etc/netz.conf",    "kernel/user/dhcp.fi"),
}

# EVERY FILE THIS PROGRAM GENERATES. Activation deletes all of them and
# then writes the ones the plan asks for. Without the complete list a
# setting that is REMOVED in a new generation would leave its file behind,
# and the tree would no longer be a function of the plan. Files under
# `etc/` that are not in this list are never touched.
GENERATED_FILES = ("etc/hostname", "etc/keymap", "etc/netz.conf",
                   "etc/passwd", "etc/schirm.conf", "etc/shadow",
                   "etc/timezone", "etc/zeit.conf")

# Where the kernel of a generation becomes visible in the tree.
KERNEL_DIR = "system/kernel"
SECRET_DIR = "system/secrets"

ACCOUNT_FIELDS = ("uid", "gid", "home", "shell", "secret")


def reserved_name(name):
    """True for the five names that would make an old PLAN ambiguous."""
    return name in PLAN_TYPES


def _clean(value, what):
    if "\t" in value or "\n" in value:
        raise ValueError("%s must not contain a tab or a newline: %r"
                         % (what, value))
    return value


class Account(object):
    __slots__ = ("name", "uid", "gid", "home", "shell", "secret")

    def __init__(self, name, uid="1000", gid="1000", home=None,
                 shell="/bin/sh", secret="-"):
        self.name = name
        self.uid = str(uid)
        self.gid = str(gid)
        self.home = home or ("/users/" + name)
        self.shell = shell
        self.secret = secret or "-"

    def fields(self):
        return [self.name, self.uid, self.gid, self.home, self.shell,
                self.secret]

    def passwd_line(self):
        # name:x:uid:gid:description:home:shell -- the shape Osum's
        # kernel/user/pw.fi reads. The `x` means "the password is
        # elsewhere", and here it really is.
        return "%s:x:%s:%s:%s:%s:%s" % (self.name, self.uid, self.gid,
                                        self.name, self.home, self.shell)


class Plan(object):
    """The whole state of a system, as one sorted text file."""

    def __init__(self):
        self.apps = {}          # name -> full sha256
        self.kernel = None      # full sha256 or None
        self.sources = {}       # url -> ed25519 public key, hex
        self.settings = {}      # key -> value        (system level)
        self.prefs = {}         # (user, key) -> value  (user level)
        self.accounts = {}      # name -> Account
        self.legacy_lines = 0   # how many lines were read in the old shape

    # ------------------------------------------------------------ reading
    @staticmethod
    def parse(text, where="PLAN"):
        p = Plan()
        for nr, raw in enumerate(text.splitlines(), 1):
            if not raw.strip() or raw.startswith("#"):
                continue
            f = raw.split("\t")
            kind = f[0]
            if kind not in PLAN_TYPES:
                # THE COMPATIBILITY RULE, and nothing else is allowed to
                # be guessed here.
                if len(f) == 2:
                    p.apps[f[0]] = f[1]
                    p.legacy_lines += 1
                    continue
                raise ValueError("%s line %d: '%s' is not a known type and "
                                 "the line is not the old two-field shape"
                                 % (where, nr, kind))
            if kind == "app":
                if len(f) != 3:
                    raise ValueError("%s line %d: app needs name and hash"
                                     % (where, nr))
                p.apps[f[1]] = f[2]
            elif kind == "kernel":
                if len(f) != 2:
                    raise ValueError("%s line %d: kernel needs exactly one "
                                     "hash" % (where, nr))
                if p.kernel is not None and p.kernel != f[1]:
                    raise ValueError("%s line %d: a second, different kernel "
                                     "line -- a generation runs on one kernel"
                                     % (where, nr))
                p.kernel = f[1]
            elif kind == "source":
                if len(f) != 3:
                    raise ValueError("%s line %d: source needs url and public "
                                     "key" % (where, nr))
                p.sources[f[1]] = f[2]
            elif kind == "setting":
                if len(f) != 3:
                    raise ValueError("%s line %d: setting needs key and value"
                                     % (where, nr))
                p.settings[f[1]] = f[2]
            elif kind == "pref":
                if len(f) != 4:
                    raise ValueError("%s line %d: pref needs user, key and "
                                     "value" % (where, nr))
                p.prefs[(f[1], f[2])] = f[3]
            elif kind == "account":
                if len(f) != 7:
                    raise ValueError("%s line %d: account needs name, uid, "
                                     "gid, home, shell, secret (6 fields "
                                     "after the type, got %d)"
                                     % (where, nr, len(f) - 1))
                p.accounts[f[1]] = Account(f[1], f[2], f[3], f[4], f[5], f[6])
        return p

    # ------------------------------------------------------------ writing
    def lines(self):
        out = []
        for name in self.apps:
            out.append("app\t%s\t%s" % (name, self.apps[name]))
        if self.kernel:
            out.append("kernel\t%s" % self.kernel)
        for url in self.sources:
            out.append("source\t%s\t%s" % (url, self.sources[url]))
        for k in self.settings:
            out.append("setting\t%s\t%s" % (k, self.settings[k]))
        for (wer, k) in self.prefs:
            out.append("pref\t%s\t%s\t%s" % (wer, k, self.prefs[(wer, k)]))
        for name in self.accounts:
            out.append("account\t" + "\t".join(self.accounts[name].fields()))
        # SORTED BY THE BYTES OF THE WHOLE LINE. `LC_ALL=C sort -c` on the
        # finished file must pass -- that is the check from outside.
        out.sort(key=lambda z: z.encode("utf-8"))
        return out

    def text(self):
        z = self.lines()
        return ("\n".join(z) + "\n") if z else ""

    # ------------------------------------------------------------- helpers
    def copy(self):
        p = Plan()
        p.apps = dict(self.apps)
        p.kernel = self.kernel
        p.sources = dict(self.sources)
        p.settings = dict(self.settings)
        p.prefs = dict(self.prefs)
        p.accounts = {n: Account(*a.fields()) for n, a in self.accounts.items()}
        p.legacy_lines = self.legacy_lines
        return p

    def hashes(self):
        """Every store hash this plan names -- applications AND kernel."""
        out = set(self.apps.values())
        if self.kernel:
            out.add(self.kernel)
        # THE ASSETS TOO. A wallpaper the plan names must survive the
        # collector and must be fetched by `rebuild`, exactly like a
        # program -- it is a store entry and nothing about it is special.
        for (wer, k), v in self.prefs.items():
            if k in USER_BLOB_KEYS:
                out.add(v)
        return out

    def users(self):
        """Everyone the plan mentions -- as an account or as a preference."""
        return sorted(set(self.accounts) | {w for (w, _) in self.prefs})

    def prefs_of(self, wer):
        return {k: v for (w, k), v in self.prefs.items() if w == wer}

    def summary(self):
        return ("%d app(s), kernel %s, %d source(s), %d setting(s), "
                "%d account(s), %d preference(s) for %d user(s)"
                % (len(self.apps),
                   (self.kernel[:12] if self.kernel else "none"),
                   len(self.sources), len(self.settings), len(self.accounts),
                   len(self.prefs), len(self.users())))


def as_plan(thing):
    """Accept either a Plan or the old `{name: hash}` dictionary.

    The German half of this program passes dictionaries around and keeps
    doing so; this is the one place where the two shapes meet.
    """
    if isinstance(thing, Plan):
        return thing
    p = Plan()
    p.apps = dict(thing)
    return p


# ---------------------------------------------------------------------------
# Rendering the plan into files the running system reads
# ---------------------------------------------------------------------------
def render_netz(settings):
    """`/etc/netz.conf` in the shape Osum writes and reads.

    The keys in the PLAN are English (`net.mode`, value `dhcp` or
    `static`); the FILE is Osum's and keeps Osum's words (`modus=fest`).
    Translating here rather than adopting the foreign words into the plan
    keeps the format of this repository English without pretending the
    other side is.
    """
    mode = settings.get("net.mode")
    if not mode:
        return None
    if mode == "dhcp":
        return "modus=dhcp\n"
    if mode == "static":
        out = "modus=fest\n"
        out += "ip=%s\n" % settings.get("net.address", "")
        out += "maske=%s\n" % settings.get("net.netmask", "")
        out += "gateway=%s\n" % settings.get("net.gateway", "")
        return out
    raise ValueError("net.mode must be 'dhcp' or 'static', not %r" % mode)


def generated_content(plan, secrets_dir):
    """What every generated file must contain for THIS plan.

    Returns {relative path: text}. A path that is absent here is a path
    that activation deletes.
    """
    s = plan.settings
    out = {}
    for key, simple in (("hostname", "etc/hostname"),
                        ("timezone", "etc/timezone"),
                        ("keymap", "etc/keymap")):
        if key in s:
            out[simple] = s[key] + "\n"
    if "time.offset" in s:
        out["etc/zeit.conf"] = "offset=%s\n" % s["time.offset"]
    if "display.mode" in s:
        # The KEY is ours and English; the FILE and its word `modus=` are
        # Osum's and are written the way Osum reads them.
        out["etc/schirm.conf"] = "modus=%s\n" % s["display.mode"]
    netz = render_netz(s)
    if netz is not None:
        out["etc/netz.conf"] = netz
    if plan.accounts:
        names = sorted(plan.accounts)
        out["etc/passwd"] = "".join(
            plan.accounts[n].passwd_line() + "\n" for n in names)
        # /etc/shadow is written from the SECRET STORE, never from the
        # plan. An account whose secret is not on this machine is written
        # LOCKED (`!`), and that is visible in the file rather than
        # guessed at the next login.
        rows = []
        for n in names:
            acc = plan.accounts[n]
            field = "!"
            if acc.secret != "-" and secrets_dir:
                p = os.path.join(secrets_dir, n)
                if os.path.exists(p):
                    field = open(p).read().strip()
            rows.append("%s:%s:0:0:99999:7:::\n" % (n, field))
        out["etc/shadow"] = "".join(rows)
    return out


def secret_hash(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def check_pref(key, value):
    """Refuse a preference that cannot be rendered, BEFORE it becomes a
    generation. A generation that cannot be activated is worse than an
    error message."""
    if key not in USER_KEYS:
        raise ValueError(
            "'%s' is not a preference this system knows. The set is closed "
            "on purpose. Known: %s" % (key, ", ".join(sorted(USER_KEYS))))
    if key == "taskbar.edge" and value not in TASKBAR_EDGES:
        raise ValueError("taskbar.edge must be one of %s (the names in "
                         "Osum's wlibc.fi), not %r"
                         % ("/".join(TASKBAR_EDGES), value))
    if key == "taskbar.height":
        if not value.isdigit() or not 12 <= int(value) <= 200:
            raise ValueError("taskbar.height must be a number of pixels "
                             "between 12 and 200, not %r" % value)
    if key == "taskbar.autohide" and value not in ("yes", "no"):
        raise ValueError("taskbar.autohide must be 'yes' or 'no', not %r"
                         % value)
    if key in USER_BLOB_KEYS:
        if len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
            raise ValueError("%s is content and is named by its SHA-256 -- "
                             "64 hex digits, not %r" % (key, value))
    return value


def render_taskbar(prefs):
    """`taskbar.conf` -- a NEW file, therefore English keys.

    Round DESKTOP is building the taskbar that will read it
    (`kernel/user/leiste.fi`); the edge names are taken from Osum's own
    `wlibc.fi` (`e_names = "bottom\0top\0left\0right"`) rather than
    invented, and the default height 28 is `const PH` from that same file.
    All three keys are always written, so the file alone determines the
    taskbar and a missing line is never a hidden default.
    """
    if not any(k.startswith("taskbar.") for k in prefs):
        return None
    v = dict(TASKBAR_DEFAULTS)
    v.update({k: prefs[k] for k in prefs if k.startswith("taskbar.")})
    return ("edge=%s\nheight=%s\nautohide=%s\n"
            % (v["taskbar.edge"], v["taskbar.height"], v["taskbar.autohide"]))


# ---------------------------------------------------------------------------
# Die Wurzel: store, apps, users, system
# ---------------------------------------------------------------------------
class Wurzel:
    def __init__(self, pfad):
        self.p = os.path.abspath(pfad)
        self.store = os.path.join(self.p, "store")
        self.apps = os.path.join(self.p, "apps")
        self.users = os.path.join(self.p, "users")
        self.gen = os.path.join(self.p, "system", "generations")
        # Whose pots are created when the plan names no accounts at all.
        self.default_user = "justin"

    def anlegen(self):
        for d in (self.store, self.apps, self.users, self.gen):
            os.makedirs(d, exist_ok=True)
        if not os.path.exists(os.path.join(self.p, "system", "AKTUELL")):
            self.plan_schreiben(0, {}, "leer")
            self.aktuell_setzen(0)

    # ------------------------------------------------------- Generationen
    def aktuell(self):
        f = os.path.join(self.p, "system", "AKTUELL")
        if not os.path.exists(f):
            return None
        return int(open(f).read().strip())

    def aktuell_setzen(self, n):
        with open(os.path.join(self.p, "system", "AKTUELL"), "w") as f:
            f.write("%d\n" % n)

    def generationen(self):
        if not os.path.isdir(self.gen):
            return []
        return sorted(int(x) for x in os.listdir(self.gen) if x.isdigit())

    def plan(self, n):
        """The APPLICATIONS of a generation, as the old `{name: hash}`.

        Kept with the old signature because the German half of this file
        calls it everywhere. Everything a typed PLAN carries beyond the
        applications is reached through `plan_full`.
        """
        return self.plan_full(n).apps

    def plan_full(self, n):
        """The whole typed plan of generation n. Reads old files too."""
        f = os.path.join(self.gen, str(n), "PLAN")
        if not os.path.exists(f):
            return Plan()
        return Plan.parse(open(f, encoding="utf-8").read(),
                          "generation %s" % n)

    def plan_schreiben(self, n, plan, grund):
        d = os.path.join(self.gen, str(n))
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "PLAN"), "w", encoding="utf-8") as f:
            f.write(as_plan(plan).text())
        with open(os.path.join(d, "GRUND"), "w") as f:
            f.write(grund + "\n")

    # ------------------------------------------------------ secret store
    @property
    def secrets(self):
        return os.path.join(self.p, SECRET_DIR)

    def secret_path(self, name):
        return os.path.join(self.secrets, name)

    def neue_generation(self, plan, grund):
        vorhanden = self.generationen()
        n = (max(vorhanden) if vorhanden else -1) + 1
        self.plan_schreiben(n, plan, grund)
        return n

    # -------------------------------------------------------- Aktivieren
    def aktivieren(self, n):
        """Die aktivierte Sicht wird VOLLSTAENDIG neu gebaut.

        Nicht schrittweise nachgezogen: ein Zustand, den man fortschreibt,
        haengt von seiner Geschichte ab, und dann ist „Generation 7" nicht
        mehr dasselbe wie „Generation 7, auf dem anderen Weg erreicht".
        Neu bauen kostet Verzeichniseintraege und keinen einzigen Block --
        die Dateien sind harte Verweise.
        """
        plan = self.plan_full(n)
        if os.path.isdir(self.apps):
            shutil.rmtree(self.apps)
        os.makedirs(self.apps, exist_ok=True)
        for name in sorted(plan.apps):
            h = plan.apps[name]
            quelle = os.path.join(self.store, kurz(h))
            if not os.path.isdir(quelle):
                raise SystemExit("opk: Generation %d nennt %s (%s), "
                                 "aber der Store hat den Eintrag nicht"
                                 % (n, name, h[:12]))
            self._verweisen(quelle, os.path.join(self.apps, name + ".prog"))
        # THE KERNEL OF THIS GENERATION (round PLAN2). It is a store entry
        # like any other and becomes visible under `system/kernel` by the
        # same hard links -- so rolling back rolls the kernel back with
        # everything else, in the same single operation, and there is no
        # second mechanism to keep in step. What that does NOT yet do is
        # in docs/ROUND-PLAN2.md, under the heading it deserves.
        kdir = os.path.join(self.p, KERNEL_DIR)
        if os.path.isdir(kdir):
            shutil.rmtree(kdir)
        if plan.kernel:
            quelle = os.path.join(self.store, kurz(plan.kernel))
            if not os.path.isdir(quelle):
                raise SystemExit("opk: Generation %d nennt den Kernel %s, "
                                 "aber der Store hat den Eintrag nicht"
                                 % (n, plan.kernel[:12]))
            self._verweisen(quelle, kdir)
        self._render_generated(plan)
        self._make_pots(plan)
        self.aktuell_setzen(n)

    def _blob_of(self, h):
        """Where the octets of an asset package sit in the store."""
        return os.path.join(self.store, kurz(h), "blob")

    def _render_user(self, plan):
        """Everything the plan determines BELOW users/, and nothing else."""
        vorhanden = sorted(os.listdir(self.users)) if os.path.isdir(self.users) \
            else []
        for wer in set(vorhanden) | set(plan.users()):
            for rel in USER_GENERATED:
                p = os.path.join(self.users, wer, rel)
                if os.path.lexists(p):
                    os.remove(p)
        for wer in plan.users():
            prefs = plan.prefs_of(wer)
            ziel = os.path.join(self.users, wer, "config", "desktop")
            if prefs:
                os.makedirs(ziel, exist_ok=True)
            for k in USER_BLOB_KEYS:
                if k not in prefs:
                    continue
                quelle = self._blob_of(prefs[k])
                if not os.path.exists(quelle):
                    raise SystemExit(
                        "opk: %s of %s is %s, and the store has no such asset"
                        % (k, wer, prefs[k][:12]))
                os.link(quelle, os.path.join(ziel, k))
            tb = render_taskbar(prefs)
            if tb is not None:
                p = os.path.join(ziel, "taskbar.conf")
                with open(p, "w", encoding="utf-8") as f:
                    f.write(tb)
                os.chmod(p, 0o644)

    def _render_compat(self, plan):
        """The /etc view Osum's programs still read. Second names, not copies.

        Only for a machine with exactly ONE account. With two accounts
        there is no honest answer to "whose theme is /etc/theme", and the
        view is therefore absent rather than arbitrary.
        """
        for rel in COMPAT_FILES:
            p = os.path.join(self.p, rel)
            if os.path.lexists(p):
                os.remove(p)
        sd = os.path.join(self.p, SCHEMA_DIR)
        if os.path.isdir(sd):
            shutil.rmtree(sd)
        # /etc/schemas: one entry per colour scheme any user of this plan
        # has chosen, named after the asset package. It is what Osum's
        # settings application lists.
        themen = {v for (w, k), v in plan.prefs.items() if k == "theme"}
        if themen:
            os.makedirs(sd, exist_ok=True)
            for h in sorted(themen):
                meta = os.path.join(self.store, kurz(h), "PAKET")
                name = h[:12]
                if os.path.exists(meta):
                    felder, _, _ = meta_lesen(open(meta, "rb").read())
                    name = felder.get("name") or name
                os.link(self._blob_of(h), os.path.join(sd, name))
        wer = list(plan.accounts)
        if len(wer) != 1:
            return len(wer)
        prefs = plan.prefs_of(wer[0])
        paare = (("theme", "etc/theme"), ("wallpaper", "etc/hintergrund"))
        for k, rel in paare:
            if k in prefs:
                os.link(self._blob_of(prefs[k]),
                        os.path.join(self.p, rel))
        tb = render_taskbar(prefs)
        if tb is not None:
            p = os.path.join(self.p, "etc/taskbar.conf")
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "w", encoding="utf-8") as f:
                f.write(tb)
            os.chmod(p, 0o644)
        return 1

    def _render_generated(self, plan):
        """Write the files that are DERIVED from the plan, and only those.

        Every path in GENERATED_FILES is removed first, so that a setting
        which disappears in a new generation takes its file with it.
        Anything else under `etc/` is left alone -- this program owns a
        fixed, named list of files and not a directory.
        """
        for rel in GENERATED_FILES:
            p = os.path.join(self.p, rel)
            if os.path.exists(p):
                os.remove(p)
        content = generated_content(plan, self.secrets)
        for rel in sorted(content):
            p = os.path.join(self.p, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "w", encoding="utf-8") as f:
                f.write(content[rel])
            # /etc/shadow is the one file here that must not be readable
            # by everyone -- the whole point of splitting it off passwd.
            os.chmod(p, 0o600 if rel == "etc/shadow" else 0o644)
        self._render_user(plan)
        self._render_compat(plan)

    def _make_pots(self, plan):
        """The three pots, for every account of the plan and every app.

        Deriving them from the plan rather than creating them as a side
        effect of `installieren` is what makes the tree a FUNCTION of the
        plan: two roots with the same plan have the same pots, no matter
        in which order they got there.
        """
        wer = plan.users() or [self.default_user]
        for nutzer in wer:
            for topf in ("config", "state", "cache"):
                for name in sorted(plan.apps):
                    os.makedirs(os.path.join(self.users, nutzer, topf, name),
                                exist_ok=True)

    def _verweisen(self, quelle, ziel):
        os.makedirs(ziel, exist_ok=True)
        for eintrag in sorted(os.listdir(quelle)):
            if eintrag == "PAKET":
                continue          # die Metadaten bleiben im Store
            q = os.path.join(quelle, eintrag)
            z = os.path.join(ziel, eintrag)
            if os.path.isdir(q):
                self._verweisen(q, z)
            else:
                os.link(q, z)

    # ------------------------------------------------------------- Store
    def store_schreiben(self, h, meta, daten):
        d = os.path.join(self.store, kurz(h))
        if os.path.isdir(d):
            # Gleicher kurzer Name -- aber ist es auch derselbe Inhalt?
            voll = self.store_hash(kurz(h))
            if voll != h:
                raise SystemExit(
                    "opk: store/%s ist belegt von %s, das neue Paket ist %s "
                    "-- achtzig Bit sind kollidiert. Das ist ein Fehler und "
                    "kein Zufall, den man uebergeht." % (kurz(h), voll[:24], h[:24]))
            return False          # schon da: gleicher Hash = gleicher Inhalt
        tmp = d + ".teil"
        if os.path.isdir(tmp):
            shutil.rmtree(tmp)
        os.makedirs(tmp)
        for typ, name, modus, inhalt in archiv_lesen(daten):
            p = os.path.join(tmp, name)
            if typ == "d":
                os.makedirs(p, exist_ok=True)
            else:
                os.makedirs(os.path.dirname(p), exist_ok=True)
                with open(p, "wb") as f:
                    f.write(inhalt)
                os.chmod(p, modus)
        with open(os.path.join(tmp, "PAKET"), "wb") as f:
            f.write(meta)
        os.chmod(os.path.join(tmp, "PAKET"), 0o644)
        # DER LETZTE SCHRITT IST EIN EINZIGES UMBENENNEN. Es gibt keinen
        # Zustand, in dem `store/<hash>` halb da ist -- PACKAGING.md § 4,
        # „Atomar".
        os.rename(tmp, d)
        return True

    def store_hash(self, kurzname):
        """Den VOLLEN Hash eines Store-Eintrags nachrechnen."""
        d = os.path.join(self.store, kurzname)
        meta = open(os.path.join(d, "PAKET"), "rb").read()
        tmp = os.path.join(self.p, ".pruef")
        if os.path.isdir(tmp):
            shutil.rmtree(tmp)
        shutil.copytree(d, tmp)
        os.remove(os.path.join(tmp, "PAKET"))
        daten = archiv_bauen(tmp)
        shutil.rmtree(tmp)
        return hashlib.sha256(meta + daten).hexdigest()

    def erreichbar(self):
        """Jeder Hash, den IRGENDEINE Generation nennt.

        Erreichbarkeit und nicht Referenzzaehlung -- PACKAGING.md § 8
        laesst die Frage offen und nennt Erreichbarkeit robuster. Sie ist
        hier auch die billigere: der ganze Zustand sind ein paar
        Textdateien.
        """
        aus = set()
        for n in self.generationen():
            # `hashes()` and not `apps.values()`: the kernel of an old
            # generation must survive the collector too, or rolling back
            # to it would find an empty store.
            aus |= self.plan_full(n).hashes()
        return aus

    def erreichbar_kurz(self):
        return {kurz(h) for h in self.erreichbar()}


def baum_liste(wurzel, ohne=()):
    """Eine kanonische Beschreibung eines Verzeichnisbaums.

    WAS HIER DRINSTEHT, IST DIE FRAGE. Ein Vergleich, der nur Pfadnamen
    liest, geht bei geaendertem Inhalt gruen durch; einer, der die ganze
    `stat` mitnimmt, ist nach jedem Kopiervorgang rot. Deshalb genau:
    Art, Pfad, Modus, Groesse und die SHA-256 des Inhalts.
    """
    zeilen = []
    ohne = set(ohne)
    for pfad, verz, dateien in os.walk(wurzel):
        rel = os.path.relpath(pfad, wurzel).replace(os.sep, "/")
        verz.sort()
        if rel == ".":
            verz[:] = [v for v in verz if v not in ohne]
        else:
            zeilen.append("d %s %o" % (rel, os.stat(pfad).st_mode & 0o7777))
        for d in sorted(dateien):
            p = os.path.join(pfad, d)
            r = os.path.relpath(p, wurzel).replace(os.sep, "/")
            st = os.lstat(p)
            h = hashlib.sha256(open(p, "rb").read()).hexdigest()
            zeilen.append("f %s %o %d %s" % (r, st.st_mode & 0o7777, st.st_size, h))
    zeilen.sort()
    return "\n".join(zeilen) + ("\n" if zeilen else "")


def inode_karte(wurzel):
    """Welche Pfade sich einen Inode teilen. Fuer die Messung der Verweise."""
    aus = {}
    for pfad, _, dateien in os.walk(wurzel):
        for d in dateien:
            p = os.path.join(pfad, d)
            st = os.lstat(p)
            aus.setdefault(st.st_ino, []).append(
                os.path.relpath(p, wurzel).replace(os.sep, "/"))
    return aus


# ---------------------------------------------------------------------------
# Rezepte -- woraus ein Paket gebaut wird
# ---------------------------------------------------------------------------
def rezept_lesen(pfad):
    felder, dateien = {}, []
    for zeile in open(pfad, encoding="utf-8"):
        z = zeile.split("#")[0].strip()
        if not z:
            continue
        k, _, v = z.partition("=")
        k, v = k.strip(), v.strip()
        if k == "datei":
            ziel, _, quelle = v.partition(" ")
            dateien.append((ziel.strip(), quelle.strip()))
        elif k in ("braucht", "handle"):
            felder.setdefault(k, []).append(v)
        else:
            felder[k] = v
    return felder, dateien


def bauen(args):
    felder, dateien = rezept_lesen(args.rezept)
    # A package called `app`, `kernel`, `source`, `setting` or `account`
    # would make an old-shape PLAN line unreadable (see PLAN v2 above).
    # Refusing it here is the last moment at which it costs nothing.
    if reserved_name(felder.get("name", "")):
        raise SystemExit("opk: '%s' is one of the five PLAN types and cannot "
                         "be a package name -- a line `%s<TAB><hash>` could "
                         "not be told apart from a typed line"
                         % (felder["name"], felder["name"]))
    basis = os.path.dirname(os.path.abspath(args.rezept))
    arbeit = os.path.join(os.path.dirname(os.path.abspath(args.aus)),
                          ".bau-" + felder["name"])
    if os.path.isdir(arbeit):
        shutil.rmtree(arbeit)
    os.makedirs(arbeit)
    fehlt = []
    for ziel, quelle in dateien:
        q = quelle if os.path.isabs(quelle) else os.path.join(basis, quelle)
        q = os.path.normpath(q)
        if not os.path.exists(q):
            fehlt.append(quelle)
            continue
        z = os.path.join(arbeit, ziel)
        os.makedirs(os.path.dirname(z), exist_ok=True)
        shutil.copyfile(q, z)
    if fehlt:
        shutil.rmtree(arbeit)
        raise SystemExit("opk: das Rezept nennt, was es nicht gibt: %s"
                         % " ".join(fehlt))
    meta = meta_bauen(felder, felder.get("braucht", []), felder.get("handle", []))
    daten = archiv_bauen(arbeit)
    shutil.rmtree(arbeit)
    roh, h = paket_packen(meta, daten)
    with open(args.aus, "wb") as f:
        f.write(roh)
    print("%-12s %-8s %8d Oktette  %s" % (felder["name"], felder.get("fassung", "-"),
                                          len(roh), h))
    return 0


def zeigen(args):
    roh = open(args.datei, "rb").read()
    meta, daten, h = paket_lesen(roh)
    felder, braucht, handles = meta_lesen(meta)
    print("hash     %s" % h)
    print("groesse  %d Oktette (Metadaten %d, Daten %d)"
          % (len(roh), len(meta), len(daten)))
    for k in FELDER:
        print("%-8s %s" % (k, felder.get(k, "")))
    if braucht:
        print("braucht  %s" % ", ".join(braucht))
    if handles:
        print("handles  %s" % ", ".join(handles))
    print("inhalt:")
    for typ, name, modus, inhalt in archiv_lesen(daten):
        print("  %s %-28s %o %8d" % (typ, name, modus, len(inhalt)))
    return 0


# ---------------------------------------------------------------------------
# Quelle (Roadmap 6.2)
# ---------------------------------------------------------------------------
def index_bauen(verzeichnis):
    zeilen = []
    for datei in sorted(os.listdir(verzeichnis)):
        if not datei.endswith(".opk"):
            continue
        roh = open(os.path.join(verzeichnis, datei), "rb").read()
        meta, daten, h = paket_lesen(roh)
        felder, _, _ = meta_lesen(meta)
        zeilen.append("%s\t%s\t%s\t%d\t%s"
                      % (felder["name"], felder.get("fassung", "-"), h,
                         len(roh), datei))
    return ("\n".join(zeilen) + "\n").encode("utf-8") if zeilen else b""


def schluessel(args):
    os.makedirs(args.verzeichnis, exist_ok=True)
    sk = os.urandom(32)
    pk = ed25519_public(sk)
    geheim = os.path.join(args.verzeichnis, "geheim.key")
    oeffentlich = os.path.join(args.verzeichnis, "oeffentlich.key")
    with open(geheim, "wb") as f:
        f.write(sk)
    os.chmod(geheim, 0o600)
    with open(oeffentlich, "wb") as f:
        f.write(pk)
    print("geheim      %s" % geheim)
    print("oeffentlich %s (%s)" % (oeffentlich, pk.hex()))
    return 0


def quelle(args):
    idx = index_bauen(args.verzeichnis)
    with open(os.path.join(args.verzeichnis, "INDEX"), "wb") as f:
        f.write(idx)
    n = len(idx.decode().strip().splitlines()) if idx.strip() else 0
    if args.schluessel:
        sk = open(args.schluessel, "rb").read()
        sig = ed25519_sign(sk, idx)
        pk = ed25519_public(sk)
        with open(os.path.join(args.verzeichnis, "INDEX.sig"), "wb") as f:
            f.write(sig)
        with open(os.path.join(args.verzeichnis, "oeffentlich.key"), "wb") as f:
            f.write(pk)
        # Die zweite Meinung, sofort: eine Signatur, die die eigene
        # Umsetzung erzeugt und nur die eigene Umsetzung glaubt, sagt
        # nichts.
        eigen = ed25519_verify(pk, idx, sig)
        fremd = ed25519_verify_bibliothek(pk, idx, sig)
        print("INDEX      %d Eintraege, %d Oktette" % (n, len(idx)))
        print("signiert   %s..." % sig.hex()[:32])
        print("geprueft   eigene Umsetzung=%s, Bibliothek=%s" % (eigen, fremd))
        if not eigen or fremd is False:
            raise SystemExit("opk: die eigene Signatur wird nicht anerkannt")
    else:
        print("INDEX      %d Eintraege, %d Oktette (NICHT signiert)" % (n, len(idx)))
    return 0


def quelle_lesen(verzeichnis, schluessel_datei=None):
    """Den Index einer Quelle lesen -- und die Signatur PRUEFEN.

    Ohne oeffentlichen Schluessel wird nicht heimlich weitergemacht: dann
    gilt die Quelle als unsigniert, und das steht in der Ausgabe.
    """
    idx = open(os.path.join(verzeichnis, "INDEX"), "rb").read()
    sig_pfad = os.path.join(verzeichnis, "INDEX.sig")
    pk_pfad = schluessel_datei or os.path.join(verzeichnis, "oeffentlich.key")
    signiert = False
    if os.path.exists(sig_pfad) and os.path.exists(pk_pfad):
        sig = open(sig_pfad, "rb").read()
        pk = open(pk_pfad, "rb").read()
        eigen = ed25519_verify(pk, idx, sig)
        fremd = ed25519_verify_bibliothek(pk, idx, sig)
        if fremd is not None and fremd != eigen:
            raise SystemExit("opk: die beiden Ed25519-Umsetzungen sind sich "
                             "uneins (eigen=%s, Bibliothek=%s) -- das ist ein "
                             "Fehler in einer von beiden, kein Paketproblem"
                             % (eigen, fremd))
        if not eigen:
            raise SystemExit("opk: die Signatur des Index passt nicht")
        signiert = True
    eintraege = {}
    for z in idx.decode("utf-8").splitlines():
        if not z.strip():
            continue
        name, fassung, h, groesse, datei = z.split("\t")
        eintraege[name] = (fassung, h, int(groesse), datei)
    return eintraege, signiert


# ---------------------------------------------------------------------------
# Die Befehle am System
# ---------------------------------------------------------------------------
def _paket_holen(args):
    """Gibt (roh, herkunft, erwarteter_hash|None)."""
    if args.was.endswith(".opk") and os.path.exists(args.was):
        return open(args.was, "rb").read(), args.was, None
    if not args.quelle:
        raise SystemExit("opk: '%s' ist keine Datei, und es ist keine Quelle "
                         "genannt (--quelle)" % args.was)
    eintraege, signiert = quelle_lesen(args.quelle, args.schluessel)
    if args.was not in eintraege:
        raise SystemExit("opk: die Quelle kennt '%s' nicht" % args.was)
    fassung, h, groesse, datei = eintraege[args.was]
    roh = open(os.path.join(args.quelle, datei), "rb").read()
    if not signiert:
        print("   ACHTUNG: die Quelle ist NICHT signiert")
    return roh, "%s (%s)" % (args.quelle,
                             "signiert" if signiert else "unsigniert"), h


def installieren(args):
    w = Wurzel(args.wurzel)
    w.anlegen()
    roh, herkunft, erwartet = _paket_holen(args)
    meta, daten, h = paket_lesen(roh)      # <- hier faellt eine falsche Summe
    if erwartet is not None and erwartet != h:
        raise SystemExit("opk: der Index nennt %s, die Datei ist %s"
                         % (erwartet[:16], h[:16]))
    felder, braucht, handles = meta_lesen(meta)
    name = felder["name"]
    if reserved_name(name):
        raise SystemExit("opk: '%s' is one of the five PLAN types and cannot "
                         "be installed under that name" % name)
    if felder.get("kind") == "kernel":
        raise SystemExit("opk: %s is a kernel package -- it belongs to the "
                         "generation, not to /apps. Use `opk.py kernel`."
                         % name)

    voll = w.plan_full(w.aktuell()).copy()
    plan = voll.apps
    # Abhaengigkeiten: jeder Name in `braucht` muss schon installiert
    # sein. Es wird NICHT nachgeladen -- was fehlt, wird benannt.
    fehlend = [b for b in braucht if b.split(" ")[0] not in plan]
    if fehlend:
        raise SystemExit("opk: %s braucht, was nicht installiert ist: %s"
                         % (name, ", ".join(fehlend)))
    neu = w.store_schreiben(h, meta, daten)
    if plan.get(name) == h:
        print("%s %s ist schon in dieser Fassung da (%s)"
              % (name, felder.get("fassung", "-"), h[:12]))
        return 0
    alt = plan.get(name)
    plan[name] = h
    n = w.neue_generation(voll, "installiert %s %s"
                          % (name, felder.get("fassung", "-")))
    w.aktivieren(n)
    for topf in ("config", "state", "cache"):
        os.makedirs(os.path.join(w.users, args.nutzer, topf, name), exist_ok=True)
    print("%s %s -> store/%s  %s"
          % (name, felder.get("fassung", "-"), h[:12],
             "neu" if neu else "war schon im Store"))
    if alt:
        print("   ersetzt %s" % alt[:12])
    print("   Generation %d, aus %s" % (n, herkunft))
    return 0


def entfernen(args):
    w = Wurzel(args.wurzel)
    w.default_user = getattr(args, "nutzer", None) or w.default_user
    voll = w.plan_full(w.aktuell()).copy()
    plan = voll.apps
    if args.was not in plan:
        raise SystemExit("opk: '%s' ist nicht installiert" % args.was)
    # Was von diesem Paket abhaengt, darf nicht ins Leere zeigen.
    haengt = []
    for name, hh in plan.items():
        if name == args.was:
            continue
        meta = open(os.path.join(w.store, kurz(hh), "PAKET"), "rb").read()
        _, braucht, _ = meta_lesen(meta)
        if any(b.split(" ")[0] == args.was for b in braucht):
            haengt.append(name)
    if haengt and not args.trotzdem:
        raise SystemExit("opk: %s wird gebraucht von: %s (--trotzdem erzwingt es)"
                         % (args.was, ", ".join(sorted(haengt))))
    h = plan.pop(args.was)
    n = w.neue_generation(voll, "entfernt %s" % args.was)
    w.aktivieren(n)
    # DIE DREI TOEPFE. PACKAGING.md § 1.3: entfernen heisst Verzeichnisse
    # loeschen, und weil es keinen fuenften Ort gibt, kann nichts
    # uebrigbleiben.
    #
    # UND HIER STEHT DIE UNANGENEHME STELLE DIESES ENTWURFS, statt in
    # einer Fussnote: Nutzerdaten sind KEIN Teil einer Generation. Ein
    # Ruecksprung auf gestern darf die Dokumente von heute nicht
    # wegnehmen -- deshalb rollt `zurueck` `users/` nicht mit. Die Kehrseite
    # ist, dass ein `entfernen` die Daten wirklich wegwirft und ein
    # spaeteres `zurueck` sie NICHT wiederbringt. Wer das nicht will,
    # nimmt `--behalte-daten`; dann bleiben die drei Toepfe stehen und
    # der Entwurf gibt an dieser Stelle nach.
    weg = 0
    nutzer_liste = sorted(os.listdir(w.users)) if os.path.isdir(w.users) else []
    if not args.behalte_daten:
        for nutzer in nutzer_liste:
            for topf in ("config", "state", "cache"):
                d = os.path.join(w.users, nutzer, topf, args.was)
                if os.path.isdir(d):
                    shutil.rmtree(d)
                    weg += 1
    print("%s entfernt, Generation %d" % (args.was, n))
    if args.behalte_daten:
        print("   Nutzerdaten BLEIBEN (--behalte-daten)")
    else:
        print("   %d Nutzerverzeichnis(se) geloescht "
              "(--behalte-daten haette sie stehengelassen)" % weg)
    print("   store/%s bleibt, solange eine Generation ihn nennt "
          "(aufraeumen loescht ihn)" % h[:12])
    return 0


def liste(args):
    w = Wurzel(args.wurzel)
    n = w.aktuell()
    plan = w.plan(n)
    print("Generation %d, %d Paket(e)" % (n, len(plan)))
    for name in sorted(plan):
        h = plan[name]
        meta = open(os.path.join(w.store, kurz(h), "PAKET"), "rb").read()
        felder, braucht, _ = meta_lesen(meta)
        print("  %-12s %-8s %s  %s" % (name, felder.get("fassung", "-"),
                                       h[:12], felder.get("titel", "")))
        if braucht:
            print("               braucht %s" % ", ".join(braucht))
    return 0


def aktualisieren(args):
    w = Wurzel(args.wurzel)
    if not args.quelle:
        raise SystemExit("opk: aktualisieren braucht eine --quelle")
    eintraege, signiert = quelle_lesen(args.quelle, args.schluessel)
    plan = w.plan(w.aktuell())
    namen = [args.was] if args.was else sorted(plan)
    getan = 0
    for name in namen:
        if name not in plan:
            print("  %s ist nicht installiert" % name)
            continue
        if name not in eintraege:
            print("  %s kennt die Quelle nicht" % name)
            continue
        fassung, h, groesse, datei = eintraege[name]
        if h == plan[name]:
            print("  %s %s ist aktuell" % (name, fassung))
            continue
        args2 = argparse.Namespace(**vars(args))
        args2.was = name
        installieren(args2)
        getan += 1
    print("%d Paket(e) erneuert (Quelle %s)"
          % (getan, "signiert" if signiert else "UNSIGNIERT"))
    return 0


def generationen(args):
    w = Wurzel(args.wurzel)
    a = w.aktuell()
    for n in w.generationen():
        plan = w.plan_full(n)
        grund = open(os.path.join(w.gen, str(n), "GRUND")).read().strip()
        # The kernel is shown per generation because that is the point of
        # round PLAN2: a kernel change is a generation, and one must be
        # able to SEE which one to go back to.
        print("%s %3d  %2d Paket(e)  kernel %-12s  %s"
              % ("*" if n == a else " ", n, len(plan.apps),
                 (plan.kernel[:12] if plan.kernel else "-"), grund))
    return 0


def zurueck(args):
    w = Wurzel(args.wurzel)
    n = int(args.n)
    if n not in w.generationen():
        raise SystemExit("opk: Generation %d gibt es nicht" % n)
    vorher = w.aktuell()
    altplan = w.plan_full(vorher)
    w.aktivieren(n)
    plan = w.plan_full(n)
    print("zurueck auf Generation %d (war %d): %d Paket(e) (%s)"
          % (n, vorher, len(plan.apps), ", ".join(sorted(plan.apps)) or "keines"))
    if altplan.kernel != plan.kernel:
        print("   KERNEL zurueckgestellt: %s -> %s (system/kernel/image)"
              % ((altplan.kernel or "keiner")[:12],
                 (plan.kernel or "keiner")[:12]))
    if altplan.settings != plan.settings:
        print("   Einstellungen zurueckgestellt: %d -> %d, die erzeugten "
              "Dateien unter etc/ sind neu geschrieben"
              % (len(altplan.settings), len(plan.settings)))
    # WAS NICHT MITROLLT, und das steht hier, weil es sonst niemand
    # bemerkt, bevor es weh tut:
    print("   apps/  ist der Stand dieser Generation, vollstaendig neu gebaut")
    print("   store/ behaelt jeden Eintrag, den IRGENDEINE Generation nennt")
    print("   users/ rollt NICHT mit -- Nutzerdaten gehoeren keiner Generation")
    return 0


def aufraeumen(args):
    w = Wurzel(args.wurzel)
    a = w.aktuell()
    alle = w.generationen()
    behalten = set(sorted(alle)[-args.behalte:]) | {a}
    weg_gen = [n for n in alle if n not in behalten]
    for n in weg_gen:
        shutil.rmtree(os.path.join(w.gen, str(n)))
    erreichbar = w.erreichbar_kurz()
    weg_store = []
    vorhanden = sorted(os.listdir(w.store)) if os.path.isdir(w.store) else []
    for h in vorhanden:
        if h not in erreichbar:
            shutil.rmtree(os.path.join(w.store, h))
            weg_store.append(h)
    print("%d Generation(en) geloescht, %d Store-Eintrag/Eintraege"
          % (len(weg_gen), len(weg_store)))
    for h in weg_store:
        print("   store/%s" % h[:12])
    return 0


def pruefen(args):
    """Jeden Store-Eintrag NACHRECHNEN.

    Das ist die Pruefung, die auch als root nicht zu umgehen ist: die
    Modusbits halten niemanden auf, der alles darf -- der Hash schon.
    """
    w = Wurzel(args.wurzel)
    fehler = 0
    eintraege = sorted(os.listdir(w.store)) if os.path.isdir(w.store) else []
    genannt = {kurz(h): h for h in w.erreichbar()}
    for k in eintraege:
        ist = w.store_hash(k)
        if not ist.startswith(k):
            print("KAPUTT store/%s -- der Inhalt rechnet sich zu %s" % (k, ist[:24]))
            fehler += 1
        elif k in genannt and genannt[k] != ist:
            print("KAPUTT store/%s -- eine Generation nennt %s, gerechnet %s"
                  % (k, genannt[k][:24], ist[:24]))
            fehler += 1
        else:
            print("ok     store/%s  (%s)" % (k, ist[:24]))
    erreichbar = w.erreichbar_kurz()
    verwaist = [h for h in eintraege if h not in erreichbar]
    fehlend = [h for h in erreichbar if h not in eintraege]
    print("%d Eintrag/Eintraege, %d kaputt, %d verwaist, %d fehlend"
          % (len(eintraege), fehler, len(verwaist), len(fehlend)))
    for h in fehlend:
        print("FEHLT  store/%s -- eine Generation nennt ihn" % h[:16])
    return 1 if (fehler or fehlend) else 0


def baum(args):
    w = Wurzel(args.wurzel)
    print(baum_liste(w.p, tuple(args.ohne or ())), end="")
    return 0


def verweise(args):
    """Welche Dateien sich einen Inode teilen -- die Messung zu § 4."""
    w = Wurzel(args.wurzel)
    karte = inode_karte(w.p)
    geteilt = {i: p for i, p in karte.items() if len(p) > 1}
    gespart = 0
    for ino in sorted(geteilt):
        pfade = sorted(geteilt[ino])
        groesse = os.lstat(os.path.join(w.p, pfade[0])).st_size
        gespart += groesse * (len(pfade) - 1)
        print("%d Namen, %8d Oktette: %s" % (len(pfade), groesse, ", ".join(pfade)))
    print("%d Datei(en) mit mehr als einem Namen, %d Oktette nicht doppelt abgelegt"
          % (len(geteilt), gespart))
    return 0


# ---------------------------------------------------------------------------
# THE COMMANDS OF ROUND PLAN2 (English)
#
# Everything below reads and writes the typed plan. The older German
# commands keep working unchanged -- they only ever touch `plan.apps`,
# which is exactly what they always touched.
# ---------------------------------------------------------------------------
def _root(args):
    w = Wurzel(args.wurzel)
    w.default_user = getattr(args, "nutzer", None) or "justin"
    return w


def _next_generation(w, plan, reason):
    n = w.neue_generation(plan, reason)
    w.aktivieren(n)
    return n


def _current(w):
    a = w.aktuell()
    if a is None:
        w.anlegen()
        a = w.aktuell()
    return w.plan_full(a)


def cmd_plan(args):
    """Print the plan of the active generation, and what it means."""
    w = _root(args)
    n = w.aktuell()
    plan = w.plan_full(n)
    f = os.path.join(w.gen, str(n), "PLAN")
    raw = open(f, "rb").read() if os.path.exists(f) else b""
    print("generation %d, %s" % (n, plan.summary()))
    print("PLAN %s, %d octet(s)"
          % (hashlib.sha256(raw).hexdigest()[:16], len(raw)))
    if plan.legacy_lines:
        print("%d line(s) were read in the OLD two-field shape "
              "(name<TAB>hash) and are treated as applications"
              % plan.legacy_lines)
    print("")
    for z in plan.lines():
        print(z.replace("\t", "  "))
    return 0


def cmd_export(args):
    """Write the active plan out as a stand-alone file.

    This is what one hands to another machine. It is the same bytes as
    `system/generations/<n>/PLAN` unless that file is still in the old
    shape -- in which case it is written out typed, which is the only
    conversion this program ever performs.
    """
    w = _root(args)
    plan = _current(w)
    text = plan.text()
    with open(args.aus, "w", encoding="utf-8") as f:
        f.write(text)
    print("%s: %d line(s), %d octet(s), %s"
          % (args.aus, len(plan.lines()), len(text.encode("utf-8")),
             plan.summary()))
    if plan.legacy_lines:
        print("   %d old-shape line(s) were written out typed"
              % plan.legacy_lines)
    if not plan.sources:
        print("   WARNING: no source line -- this plan cannot be rebuilt "
              "anywhere else. `opk.py source-add` first.")
    return 0


# ------------------------------------------------------------------ kernel
def cmd_kernel(args):
    """Make a kernel package the kernel of a NEW generation.

    A kernel change is a generation like any other. That is the whole
    point: `zurueck` puts the old kernel back with the same single
    operation that puts the old applications back, and there is no second
    piece of bookkeeping that could fall out of step.
    """
    w = _root(args)
    w.anlegen()
    plan = _current(w).copy()
    if args.was == "none":
        if plan.kernel is None:
            print("no kernel in this generation, nothing to do")
            return 0
        alt = plan.kernel
        plan.kernel = None
        n = _next_generation(w, plan, "removed kernel %s" % alt[:12])
        print("kernel removed, generation %d" % n)
        return 0
    roh, herkunft, erwartet = _paket_holen(args)
    meta, daten, h = paket_lesen(roh)
    if erwartet is not None and erwartet != h:
        raise SystemExit("opk: the index says %s, the file is %s"
                         % (erwartet[:16], h[:16]))
    felder, braucht, handles = meta_lesen(meta)
    if felder.get("kind") != "kernel":
        raise SystemExit("opk: %s is not a kernel package (kind=%s). A kernel "
                         "package carries `kind=kernel` and a file `image` -- "
                         "otherwise any application could become the kernel."
                         % (felder.get("name", "?"), felder.get("kind", "")))
    if plan.kernel == h:
        print("this kernel is already the one of generation %d (%s)"
              % (w.aktuell(), h[:12]))
        return 0
    alt = plan.kernel
    neu = w.store_schreiben(h, meta, daten)
    plan.kernel = h
    n = _next_generation(w, plan, "kernel %s %s"
                         % (felder.get("name", "?"),
                            felder.get("fassung", "-")))
    print("kernel %s %s -> store/%s  %s"
          % (felder.get("name", "?"), felder.get("fassung", "-"), h[:12],
             "new" if neu else "already in the store"))
    if felder.get("origin"):
        print("   origin  %s" % felder["origin"])
    if alt:
        print("   replaces %s -- `opk.py zurueck %d` puts it back"
              % (alt[:12], w.aktuell() - 1))
    print("   generation %d, from %s" % (n, herkunft))
    print("   system/kernel/image is now a second name for "
          "store/%s/image" % kurz(h))
    return 0


# ------------------------------------------------------------------ sources
def cmd_source_add(args):
    """Record a source AND its public key in the system state.

    The key travels WITH the plan on purpose. A plan that says only where
    to fetch from is not self-bearing: whoever controls the location
    controls the contents. A plan that carries the key is a statement
    about WHOSE packages this machine consists of, and `rebuild` refuses
    everything that is not signed by one of these keys.
    """
    w = _root(args)
    w.anlegen()
    plan = _current(w).copy()
    key = args.key
    if os.path.exists(key):
        key = open(key, "rb").read().hex()
    key = key.strip().lower()
    if len(key) != 64 or any(c not in "0123456789abcdef" for c in key):
        raise SystemExit("opk: an Ed25519 public key is 32 octets = 64 hex "
                         "digits, this is %d character(s)" % len(key))
    _clean(args.url, "a source url")
    if plan.sources.get(args.url) == key:
        print("source %s is already recorded with this key" % args.url)
        return 0
    plan.sources[args.url] = key
    n = _next_generation(w, plan, "source %s" % args.url)
    print("source %s\n   key %s...\n   generation %d"
          % (args.url, key[:32], n))
    return 0


def cmd_source_remove(args):
    w = _root(args)
    plan = _current(w).copy()
    if args.url not in plan.sources:
        raise SystemExit("opk: '%s' is not a source of this system" % args.url)
    del plan.sources[args.url]
    n = _next_generation(w, plan, "source removed %s" % args.url)
    print("source %s removed, generation %d" % (args.url, n))
    return 0


# ----------------------------------------------------------------- settings
def cmd_set(args):
    w = _root(args)
    w.anlegen()
    plan = _current(w).copy()
    if args.key not in SETTING_KEYS:
        raise SystemExit(
            "opk: '%s' is not a setting this system knows. The set is CLOSED "
            "on purpose -- an open key space is the junk drawer that a system "
            "state without a registry exists to avoid. Known: %s"
            % (args.key, ", ".join(sorted(SETTING_KEYS))))
    _clean(args.value, "a setting value")
    plan.settings[args.key] = args.value
    # Fail before writing a generation, not after: `net.mode=maybe` must
    # not become a generation that cannot be activated.
    render_netz(plan.settings)
    n = _next_generation(w, plan, "setting %s=%s" % (args.key, args.value))
    datei, consumer = SETTING_KEYS[args.key]
    print("%s = %s, generation %d" % (args.key, args.value, n))
    print("   renders %s" % datei)
    if consumer:
        print("   read in the running system by %s" % consumer)
    else:
        print("   NOTE: nothing in the running system reads this file yet -- "
              "it is recorded and rendered, and that is all it does")
    return 0


def cmd_unset(args):
    w = _root(args)
    plan = _current(w).copy()
    if args.key not in plan.settings:
        raise SystemExit("opk: '%s' is not set" % args.key)
    del plan.settings[args.key]
    n = _next_generation(w, plan, "setting removed %s" % args.key)
    print("%s unset, generation %d -- %s is gone from the tree"
          % (args.key, n, SETTING_KEYS[args.key][0]))
    return 0


def cmd_settings(args):
    w = _root(args)
    plan = _current(w)
    for k in sorted(SETTING_KEYS):
        datei, consumer = SETTING_KEYS[k]
        wert = plan.settings.get(k)
        print("%-14s %-22s %-18s %s"
              % (k, (wert if wert is not None else "-"), datei,
                 consumer or "(no consumer yet)"))
    return 0


# ----------------------------------------------------------------- accounts
def cmd_account_add(args):
    """Add or change an account. The PASSWORD is not an argument here.

    `secret-set` puts the credential into the secret store and records its
    hash in the plan; this command only describes the account.
    """
    w = _root(args)
    w.anlegen()
    plan = _current(w).copy()
    if reserved_name(args.name):
        raise SystemExit("opk: '%s' is a plan type and cannot be a name"
                         % args.name)
    alt = plan.accounts.get(args.name)
    acc = Account(args.name, args.uid, args.gid, args.home, args.shell,
                  alt.secret if alt else "-")
    for feld in acc.fields():
        _clean(feld, "an account field")
    plan.accounts[args.name] = acc
    n = _next_generation(w, plan, "account %s" % args.name)
    print("account %s uid=%s gid=%s home=%s shell=%s, generation %d"
          % (acc.name, acc.uid, acc.gid, acc.home, acc.shell, n))
    print("   etc/passwd rewritten (%d account(s))" % len(plan.accounts))
    if acc.secret == "-":
        print("   no credential -- etc/shadow carries `!` for this account, "
              "so it cannot be logged into. `opk.py secret-set` changes that.")
    return 0


def cmd_account_remove(args):
    w = _root(args)
    plan = _current(w).copy()
    if args.name not in plan.accounts:
        raise SystemExit("opk: '%s' is not an account of this system"
                         % args.name)
    del plan.accounts[args.name]
    n = _next_generation(w, plan, "account removed %s" % args.name)
    p = w.secret_path(args.name)
    if os.path.exists(p) and not args.behalte_daten:
        os.remove(p)
        print("   the credential in %s/%s was deleted" % (SECRET_DIR, args.name))
    print("account %s removed, generation %d" % (args.name, n))
    return 0


def cmd_secret_set(args):
    """Put a credential into the secret store and record its HASH.

    The plan gets `secret=<sha256 of the credential file>` and never the
    credential. Changing a password therefore IS a new generation -- it
    changes the plan -- and rolling back to an older generation restores
    the older password, provided the older credential is still in the
    store.
    """
    w = _root(args)
    plan = _current(w).copy()
    if args.name not in plan.accounts:
        raise SystemExit("opk: '%s' is not an account of this system"
                         % args.name)
    os.makedirs(w.secrets, exist_ok=True)
    os.chmod(w.secrets, 0o700)
    roh = open(args.datei, "rb").read() if args.datei != "-" \
        else sys.stdin.buffer.read()
    ziel = w.secret_path(args.name)
    with open(ziel, "wb") as f:
        f.write(roh)
    os.chmod(ziel, 0o600)
    h = secret_hash(ziel)
    plan.accounts[args.name].secret = h
    n = _next_generation(w, plan, "credential %s" % args.name)
    print("credential for %s: %d octet(s), sha256 %s..., generation %d"
          % (args.name, len(roh), h[:16], n))
    print("   the credential itself is in %s/%s (0600) and is NOT in the plan"
          % (SECRET_DIR, args.name))
    return 0


# ------------------------------------------------------------------ rebuild
def _source_location(url):
    """Where a source url can be read from on this host, or None."""
    if url.startswith("file://"):
        return url[len("file://"):]
    if url.startswith("/") or url.startswith("./"):
        return url
    return None


def _open_source(pfad, erlaubt, woher):
    """Read a source index and verify it against the keys of the PLAN.

    `erlaubt` is the set of public keys the plan names. A source whose
    index is not signed by one of THEM is refused -- not "warned about".
    That is the difference between a plan that carries its trust and a
    plan that merely carries an address.
    """
    idxp = os.path.join(pfad, "INDEX")
    sigp = os.path.join(pfad, "INDEX.sig")
    if not os.path.exists(idxp):
        return None, "%s has no INDEX" % pfad
    if not os.path.exists(sigp):
        return None, "%s has no INDEX.sig -- an unsigned source cannot be " \
                     "the base of a rebuild" % pfad
    idx = open(idxp, "rb").read()
    sig = open(sigp, "rb").read()
    passt = None
    for hexkey in sorted(erlaubt):
        pk = bytes.fromhex(hexkey)
        eigen = ed25519_verify(pk, idx, sig)
        fremd = ed25519_verify_bibliothek(pk, idx, sig)
        if fremd is not None and fremd != eigen:
            raise SystemExit("opk: the two Ed25519 implementations disagree "
                             "(own=%s, library=%s) -- that is a bug in one of "
                             "them, not a package problem" % (eigen, fremd))
        if eigen:
            passt = hexkey
            break
    if passt is None:
        return None, "%s: the index is not signed by any of the %d key(s) " \
                     "this plan names" % (pfad, len(erlaubt))
    eintraege = {}
    for z in idx.decode("utf-8").splitlines():
        if not z.strip():
            continue
        name, fassung, h, groesse, datei = z.split("\t")
        eintraege[h] = (name, fassung, int(groesse),
                        os.path.join(pfad, datei))
    return (eintraege, passt, woher), None



# ------------------------------------------------------------------ assets
def asset_bauen(datei, klasse, name, aus):
    """Wrap ANY file into an `.opk` with `kind=asset`.

    An asset is a package with one file called `blob` and no `start`. It
    is not special in any other way: same header, same hash, same store,
    same collector, same signed INDEX. That is the whole argument for
    doing it this way instead of building a second blob store beside the
    first one.
    """
    felder = {"name": name, "fassung": "1.0.0", "titel": name,
              "info": "%s asset, %d octet(s)" % (klasse,
                                                 os.path.getsize(datei)),
              "keys": name, "kind": "asset", "class": klasse}
    arbeit = os.path.join(os.path.dirname(os.path.abspath(aus)),
                          ".asset-" + name)
    if os.path.isdir(arbeit):
        shutil.rmtree(arbeit)
    os.makedirs(arbeit)
    shutil.copyfile(datei, os.path.join(arbeit, "blob"))
    meta = meta_bauen(felder, [], [])
    daten = archiv_bauen(arbeit)
    shutil.rmtree(arbeit)
    roh, h = paket_packen(meta, daten)
    with open(aus, "wb") as f:
        f.write(roh)
    return roh, h


def cmd_asset_pack(args):
    name = args.name or os.path.splitext(os.path.basename(args.datei))[0]
    if reserved_name(name):
        raise SystemExit("opk: '%s' is a PLAN type and cannot be a name" % name)
    roh, h = asset_bauen(args.datei, args.klasse, name, args.aus)
    print("%-12s %-10s %8d octet(s) in, %8d out  %s"
          % (name, args.klasse, os.path.getsize(args.datei), len(roh), h))
    return 0


def _asset_in_store(w, wert, klasse, quelle=None, schluessel=None):
    """Turn what the user typed into a hash that IS in the store.

    Three things are accepted, and the difference matters:
      * a 64-hex hash  -- must already be in the store, or fetchable
      * an `.opk` file -- read, checksum verified, put in the store
      * ANY other file -- packed into an asset package on the spot, so
        that "I picked this photo" is one command and not three
    """
    if len(wert) == 64 and all(c in "0123456789abcdef" for c in wert):
        if os.path.isdir(os.path.join(w.store, kurz(wert))):
            return wert
        if quelle:
            eintraege, signiert = quelle_lesen(quelle, schluessel)
            for name, (fassung, h, groesse, datei) in eintraege.items():
                if h != wert:
                    continue
                roh = open(os.path.join(quelle, datei), "rb").read()
                meta, daten, ist = paket_lesen(roh)
                w.store_schreiben(ist, meta, daten)
                return ist
        raise SystemExit("opk: %s is not in the store and in no named source"
                         % wert[:16])
    if not os.path.exists(wert):
        raise SystemExit("opk: '%s' is neither a hash nor a file" % wert)
    if wert.endswith(".opk"):
        roh = open(wert, "rb").read()
    else:
        name = os.path.splitext(os.path.basename(wert))[0]
        tmp = os.path.join(w.p, ".asset.opk")
        os.makedirs(w.p, exist_ok=True)
        roh, _ = asset_bauen(wert, klasse, name, tmp)
        os.remove(tmp)
    meta, daten, h = paket_lesen(roh)
    felder, _, _ = meta_lesen(meta)
    if felder.get("kind") != "asset":
        raise SystemExit("opk: %s is not an asset package (kind=%s)"
                         % (wert, felder.get("kind", "")))
    if felder.get("class") != klasse:
        raise SystemExit("opk: %s is a '%s' asset, but this is the '%s' "
                         "preference -- a wallpaper is not a colour scheme"
                         % (wert, felder.get("class", "?"), klasse))
    w.store_schreiben(h, meta, daten)
    return h


# ------------------------------------------------------------- preferences
def cmd_pref(args):
    """Set ONE preference of ONE user. The user is not optional.

    That is the point of this command existing next to `set`: a
    preference without a person is what `/etc/theme` was, and it is the
    thing this addendum is here to undo.
    """
    w = _root(args)
    w.anlegen()
    plan = _current(w).copy()
    if args.key not in USER_KEYS:
        raise SystemExit(
            "opk: '%s' is not a preference this system knows. Known: %s"
            % (args.key, ", ".join(sorted(USER_KEYS))))
    wert = args.value
    if args.key in USER_BLOB_KEYS:
        wert = _asset_in_store(w, wert, args.key, args.quelle, args.schluessel)
    try:
        check_pref(args.key, wert)
    except ValueError as e:
        raise SystemExit("opk: %s" % e)
    _clean(args.user, "a user name")
    plan.prefs[(args.user, args.key)] = wert
    n = _next_generation(w, plan, "pref %s %s=%s"
                         % (args.user, args.key, wert[:16]))
    datei, art = USER_KEYS[args.key]
    print("%s: %s = %s, generation %d" % (args.user, args.key, wert[:24], n))
    print("   renders users/%s/%s" % (args.user, datei))
    if art == "blob":
        print("   the octets are store/%s/blob -- one copy, a second name"
              % kurz(wert))
    if len(plan.accounts) == 1:
        print("   and, for this single-account machine, the compatibility "
              "view under etc/ that Osum's programs still read")
    elif len(plan.accounts) > 1:
        print("   NO compatibility view under etc/: this machine has %d "
              "accounts, and there is no honest answer to whose theme "
              "/etc/theme would be" % len(plan.accounts))
    return 0


def cmd_pref_unset(args):
    w = _root(args)
    plan = _current(w).copy()
    if (args.user, args.key) not in plan.prefs:
        raise SystemExit("opk: %s has no preference '%s'"
                         % (args.user, args.key))
    del plan.prefs[(args.user, args.key)]
    n = _next_generation(w, plan, "pref removed %s %s" % (args.user, args.key))
    print("%s: %s unset, generation %d -- users/%s/%s is gone"
          % (args.user, args.key, n, args.user, USER_KEYS[args.key][0]))
    return 0


def cmd_prefs(args):
    w = _root(args)
    plan = _current(w)
    leute = [args.user] if args.user else plan.users()
    if not leute:
        print("no user has a preference, and no account exists")
    for wer in leute:
        p = plan.prefs_of(wer)
        print("%s%s" % (wer, "" if wer in plan.accounts
                        else "   (no account -- preferences without a person "
                             "to log in as)"))
        for k in sorted(USER_KEYS):
            datei, art = USER_KEYS[k]
            wert = p.get(k, "-")
            zusatz = ""
            if art == "blob" and wert != "-":
                meta = os.path.join(w.store, kurz(wert), "PAKET")
                if os.path.exists(meta):
                    felder, _, _ = meta_lesen(open(meta, "rb").read())
                    zusatz = "  (%s)" % felder.get("name", "?")
                wert = wert[:16] + "..."
            print("   %-18s %-22s %s%s" % (k, wert, datei, zusatz))
    print("")
    print("system level (`opk.py settings`) is a different thing: resolution, "
          "network, timezone are one per MACHINE, these are one per PERSON")
    return 0


def cmd_rebuild(args):
    """Build a whole system tree FROM A PLAN, on a machine that has none.

    This is the measurement of round PLAN2. Input: one text file and one
    or more package sources. Output: a tree that is octet for octet the
    tree the plan came from -- for everything the plan determines.
    """
    text = open(args.plan, encoding="utf-8").read()
    plan = Plan.parse(text, args.plan)
    print("plan %s: %s" % (args.plan, plan.summary()))
    if plan.legacy_lines:
        print("   %d line(s) in the old two-field shape" % plan.legacy_lines)

    erlaubt = set(plan.sources.values())
    if not erlaubt:
        raise SystemExit("opk: this plan names no source and no key -- it "
                         "cannot be rebuilt anywhere. That is a property of "
                         "the plan, not a failure of this program.")

    orte = []
    for url in sorted(plan.sources):
        p = _source_location(url)
        if p is None:
            print("   source %s: this program can only read `file://` and "
                  "local paths, not %s -- skipped"
                  % (url, url.split(":")[0]))
            continue
        orte.append((p, url))
    for extra in (args.source or []):
        orte.append((extra, "--source " + extra))

    index, benutzt = {}, []
    for p, woher in orte:
        got, warum = _open_source(p, erlaubt, woher)
        if got is None:
            print("   REFUSED %s" % warum)
            continue
        eintraege, key, woher = got
        benutzt.append((woher, len(eintraege), key))
        for h, v in eintraege.items():
            index.setdefault(h, v)
    if not benutzt:
        raise SystemExit("opk: not one usable source -- nothing to rebuild "
                         "from")
    for woher, n, key in benutzt:
        print("   source %s: %d package(s), signature by %s..."
              % (woher, n, key[:16]))

    gebraucht = sorted(plan.hashes())
    fehlt = [h for h in gebraucht if h not in index]
    if fehlt:
        raise SystemExit("opk: %d of %d hash(es) are in no source: %s"
                         % (len(fehlt), len(gebraucht),
                            ", ".join(h[:12] for h in fehlt)))

    w = Wurzel(args.wurzel)
    w.default_user = args.nutzer or "justin"
    w.anlegen()
    oktette = 0
    for h in gebraucht:
        name, fassung, groesse, datei = index[h]
        roh = open(datei, "rb").read()
        meta, daten, ist = paket_lesen(roh)     # the checksum falls here
        if ist != h:
            raise SystemExit("opk: %s says it is %s but is %s"
                             % (datei, h[:16], ist[:16]))
        w.store_schreiben(ist, meta, daten)
        oktette += len(roh)
    print("   %d package(s) fetched and verified, %d octet(s)"
          % (len(gebraucht), oktette))

    if args.secrets:
        os.makedirs(w.secrets, exist_ok=True)
        os.chmod(w.secrets, 0o700)
        n_geheim = 0
        for name in sorted(plan.accounts):
            q = os.path.join(args.secrets, name)
            if not os.path.exists(q):
                continue
            ziel = w.secret_path(name)
            with open(ziel, "wb") as f:
                f.write(open(q, "rb").read())
            os.chmod(ziel, 0o600)
            ist = secret_hash(ziel)
            if ist != plan.accounts[name].secret:
                raise SystemExit(
                    "opk: the credential for %s hashes to %s, the plan says "
                    "%s -- this is not the password this plan was written "
                    "with" % (name, ist[:16], plan.accounts[name].secret[:16]))
            n_geheim += 1
        print("   %d credential(s) restored and checked against the plan"
              % n_geheim)

    n = w.neue_generation(plan, "rebuilt from %s" % os.path.basename(args.plan))
    w.aktivieren(n)
    fehlend = [a for a in sorted(plan.accounts)
               if plan.accounts[a].secret != "-"
               and not os.path.exists(w.secret_path(a))]
    print("generation %d in %s" % (n, w.p))
    if fehlend:
        print("   %d account(s) are LOCKED because their credential is not "
              "here: %s. A plan carries the hash of a password, never the "
              "password -- restore them from the backup (--secrets)."
              % (len(fehlend), ", ".join(fehlend)))
    return 0


# ----------------------------------------------------------------- snapshot
def snapshot_lines(w):
    """A canonical description of everything THE PLAN DETERMINES.

    What is in here: the plan itself, every store entry the plan names,
    the activated view, the kernel of the generation, the generated
    configuration files, and the three pots. Kind, path, mode, size and
    the SHA-256 of every content -- the same depth `baum_liste` uses,
    because a comparison that reads only path names goes green on changed
    contents.

    What is deliberately NOT in here, and this is the honest half:

      * `system/generations/*` and `system/AKTUELL`. The history of how a
        machine got here is a LOG, not state. A rebuilt machine has one
        generation where the original had forty; demanding that they match
        would be demanding that the rebuild fake a past it does not have.
      * `users/<who>/{config,state,cache}/<app>` CONTENTS -- the
        documents. A plan describes a system, not the work done on it.
        The directories are compared, what is in them is not.
      * `system/secrets/`. Credentials are not in a plan by construction.
    """
    plan = w.plan_full(w.aktuell())
    zeilen, dateien, oktette = [], 0, 0

    text = plan.text().encode("utf-8")
    zeilen.append("plan %s %d" % (hashlib.sha256(text).hexdigest(), len(text)))

    wege = []
    for h in sorted(plan.hashes()):
        wege.append(os.path.join(w.store, kurz(h)))
    wege.append(w.apps)
    wege.append(os.path.join(w.p, KERNEL_DIR))
    for rel in GENERATED_FILES:
        wege.append(os.path.join(w.p, rel))
    wege.append(os.path.join(w.p, SCHEMA_DIR))
    for rel in COMPAT_FILES:
        wege.append(os.path.join(w.p, rel))
    wer = plan.users() or [w.default_user]
    for nutzer in wer:
        for topf in ("config", "state", "cache"):
            # THE DIRECTORY, NOT WHAT IS IN IT. This was wrong in the
            # first version of this function: it walked the pots and
            # therefore compared the user's documents, which a plan does
            # not determine. It went unnoticed because the pots were
            # empty in the run that measured it. Fixed here, and named
            # here rather than quietly.
            wege.append(("dir-only", os.path.join(w.users, nutzer, topf)))
        for rel in USER_GENERATED:
            wege.append(os.path.join(w.users, nutzer, rel))

    gesehen = set()
    for weg in wege:
        nur_verzeichnisse = False
        if isinstance(weg, tuple):
            nur_verzeichnisse, weg = True, weg[1]
        if not os.path.exists(weg):
            continue
        if nur_verzeichnisse:
            paare = [(weg, True)]
            for pfad, verz, _ in os.walk(weg):
                verz.sort()
                for v in verz:
                    paare.append((os.path.join(pfad, v), True))
        elif os.path.isfile(weg):
            paare = [(weg, None)]
        else:
            paare = []
            for pfad, verz, namen in os.walk(weg):
                verz.sort()
                paare.append((pfad, True))
                for nn in sorted(namen):
                    paare.append((os.path.join(pfad, nn), None))
        for p, istverz in paare:
            rel = os.path.relpath(p, w.p).replace(os.sep, "/")
            if rel in gesehen:
                continue
            gesehen.add(rel)
            st = os.lstat(p)
            if istverz:
                zeilen.append("d %s %o" % (rel, st.st_mode & 0o7777))
            else:
                inhalt = open(p, "rb").read()
                dateien += 1
                oktette += len(inhalt)
                zeilen.append("f %s %o %d %s"
                              % (rel, st.st_mode & 0o7777, len(inhalt),
                                 hashlib.sha256(inhalt).hexdigest()))
    kopf = zeilen[0]
    rest = sorted(zeilen[1:])
    rest.append("count entries=%d files=%d octets=%d"
                % (len(rest), dateien, oktette))
    return [kopf] + rest


def cmd_snapshot(args):
    w = _root(args)
    for z in snapshot_lines(w):
        print(z)
    return 0


# ------------------------------------------------------------------- verify
def cmd_verify(args):
    """Everything `pruefen` checks, plus everything the typed plan adds.

    `pruefen` recomputes the store. This also asks whether the PLAN itself
    is well formed: sorted the way a plain `sort` would sort it, no
    unknown setting keys, a kernel that is really a kernel, keys that are
    really keys, and credentials whose hashes still match what the plan
    claims about them.
    """
    w = _root(args)
    fehler, zusagen = 0, 0

    def sagen(gut, text):
        nonlocal fehler, zusagen
        zusagen += 1
        if not gut:
            fehler += 1
        print("%s %s" % ("ok    " if gut else "FAILED", text))

    n = w.aktuell()
    f = os.path.join(w.gen, str(n), "PLAN")
    roh = open(f, "rb").read() if os.path.exists(f) else b""
    try:
        plan = Plan.parse(roh.decode("utf-8"), f)
        sagen(True, "the plan of generation %d parses: %s"
              % (n, plan.summary()))
    except ValueError as e:
        print("FAILED %s" % e)
        return 1

    zeilen = roh.decode("utf-8").splitlines()
    sortiert = sorted(zeilen, key=lambda z: z.encode("utf-8"))
    sagen(zeilen == sortiert,
          "the file is sorted by the octets of the whole line "
          "(`LC_ALL=C sort -c` agrees), %d line(s)" % len(zeilen))

    for name in sorted(plan.apps):
        sagen(not reserved_name(name),
              "the application name '%s' is not one of the five plan types"
              % name)

    for h in sorted(plan.hashes()):
        d = os.path.join(w.store, kurz(h))
        if not os.path.isdir(d):
            sagen(False, "store/%s is named by the plan and is not there"
                  % kurz(h))
            continue
        ist = w.store_hash(kurz(h))
        sagen(ist == h, "store/%s recomputes to the hash the plan names"
              % kurz(h))

    if plan.kernel:
        meta = open(os.path.join(w.store, kurz(plan.kernel), "PAKET"),
                    "rb").read()
        felder, _, _ = meta_lesen(meta)
        sagen(felder.get("kind") == "kernel",
              "the kernel entry carries kind=kernel (%s %s, origin %s)"
              % (felder.get("name", "?"), felder.get("fassung", "-"),
                 felder.get("origin", "unrecorded")))
        sagen(os.path.exists(os.path.join(w.p, KERNEL_DIR, "image")),
              "%s/image exists in the activated tree" % KERNEL_DIR)

    for url, key in sorted(plan.sources.items()):
        sagen(len(key) == 64 and all(c in "0123456789abcdef" for c in key),
              "the key of source %s is 32 octets of hex" % url)

    for k in sorted(plan.settings):
        sagen(k in SETTING_KEYS, "the setting '%s' is a key this system knows"
              % k)

    # ------------------------------------------------ the user level
    for (wer, k) in sorted(plan.prefs):
        wert = plan.prefs[(wer, k)]
        try:
            check_pref(k, wert)
            sagen(True, "the preference %s/%s is well formed (%s)"
                  % (wer, k, wert[:16]))
        except ValueError as e:
            sagen(False, "the preference %s/%s: %s" % (wer, k, e))
            continue
        if k in USER_BLOB_KEYS:
            blob = w._blob_of(wert)
            if not os.path.exists(blob):
                sagen(False, "the %s of %s names store/%s, which is not there"
                      % (k, wer, kurz(wert)))
                continue
            ziel = os.path.join(w.users, wer, USER_KEYS[k][0])
            sagen(os.path.exists(ziel)
                  and os.lstat(ziel).st_ino == os.lstat(blob).st_ino,
                  "users/%s/%s is the SAME INODE as store/%s/blob -- one copy "
                  "of %d octet(s), not two"
                  % (wer, USER_KEYS[k][0], kurz(wert),
                     os.path.getsize(blob)))
    for wer in plan.users():
        tb = render_taskbar(plan.prefs_of(wer))
        p = os.path.join(w.users, wer, "config/desktop/taskbar.conf")
        if tb is None:
            sagen(not os.path.exists(p),
                  "%s has no taskbar preference and no taskbar.conf" % wer)
        else:
            ist = open(p).read() if os.path.exists(p) else None
            sagen(ist == tb, "users/%s/config/desktop/taskbar.conf is what "
                  "the plan renders (%s)"
                  % (wer, tb.replace("\n", " ").strip()))
    # The compatibility view: present for one account, absent for two.
    n_konten = len(plan.accounts)
    hat = [rel for rel in COMPAT_FILES
           if os.path.exists(os.path.join(w.p, rel))]
    if n_konten == 1:
        wer = list(plan.accounts)[0]
        soll = []
        if "theme" in plan.prefs_of(wer):
            soll.append("etc/theme")
        if "wallpaper" in plan.prefs_of(wer):
            soll.append("etc/hintergrund")
        if render_taskbar(plan.prefs_of(wer)) is not None:
            soll.append("etc/taskbar.conf")
        sagen(sorted(hat) == sorted(soll),
              "one account -> the compatibility view under etc/ is exactly "
              "%s" % (", ".join(sorted(soll)) or "empty"))
    elif n_konten > 1:
        sagen(hat == [],
              "%d accounts -> NO compatibility view under etc/, because "
              "there is no whose-theme-is-it answer" % n_konten)

    inhalt = generated_content(plan, w.secrets)
    for rel in sorted(GENERATED_FILES):
        p = os.path.join(w.p, rel)
        if rel in inhalt:
            ist = open(p).read() if os.path.exists(p) else None
            sagen(ist == inhalt[rel],
                  "%s is what the plan renders (%d octet(s))"
                  % (rel, len(inhalt[rel])))
        else:
            sagen(not os.path.exists(p),
                  "%s is absent, as a plan without that setting requires"
                  % rel)

    for name in sorted(plan.accounts):
        acc = plan.accounts[name]
        if acc.secret == "-":
            continue
        p = w.secret_path(name)
        if not os.path.exists(p):
            print("note   the credential of %s is not on this machine -- "
                  "the account is locked, which is what a plan alone can do"
                  % name)
            continue
        sagen(secret_hash(p) == acc.secret,
              "the credential of %s hashes to what the plan says" % name)

    print("%d assertion(s), %d failed" % (zusagen, fehler))
    return 1 if fehler else 0


def main(argv):
    p = argparse.ArgumentParser(
        prog="opk.py", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    u = p.add_subparsers(dest="befehl", required=True)

    def mit_wurzel(sp):
        # `--root` is the English spelling of the same flag. New commands
        # use it; the old ones accept both, so nothing has to be retyped.
        sp.add_argument("--wurzel", "--root", dest="wurzel", required=True)
        sp.add_argument("--nutzer", default="justin")
        sp.add_argument("--quelle", default=None)
        sp.add_argument("--schluessel", default=None)
        return sp

    s = u.add_parser("bauen"); s.add_argument("rezept")
    s.add_argument("-o", "--aus", required=True); s.set_defaults(f=bauen)
    s = u.add_parser("zeigen"); s.add_argument("datei"); s.set_defaults(f=zeigen)
    s = mit_wurzel(u.add_parser("installieren")); s.add_argument("was")
    s.set_defaults(f=installieren)
    s = mit_wurzel(u.add_parser("entfernen")); s.add_argument("was")
    s.add_argument("--trotzdem", action="store_true")
    s.add_argument("--behalte-daten", dest="behalte_daten", action="store_true")
    s.set_defaults(f=entfernen)
    s = mit_wurzel(u.add_parser("liste")); s.set_defaults(f=liste)
    s = mit_wurzel(u.add_parser("aktualisieren")); s.add_argument("was", nargs="?")
    s.set_defaults(f=aktualisieren)
    s = mit_wurzel(u.add_parser("generationen")); s.set_defaults(f=generationen)
    s = mit_wurzel(u.add_parser("zurueck")); s.add_argument("n"); s.set_defaults(f=zurueck)
    s = mit_wurzel(u.add_parser("aufraeumen"))
    s.add_argument("--behalte", type=int, default=1); s.set_defaults(f=aufraeumen)
    s = mit_wurzel(u.add_parser("pruefen")); s.set_defaults(f=pruefen)
    s = mit_wurzel(u.add_parser("baum")); s.add_argument("--ohne", action="append")
    s.set_defaults(f=baum)
    s = mit_wurzel(u.add_parser("verweise")); s.set_defaults(f=verweise)
    s = u.add_parser("schluessel"); s.add_argument("verzeichnis")
    s.set_defaults(f=schluessel)
    s = u.add_parser("quelle"); s.add_argument("verzeichnis")
    s.add_argument("--schluessel", default=None); s.set_defaults(f=quelle)

    # ------------------------------------------------ round PLAN2 (English)
    s = mit_wurzel(u.add_parser("plan")); s.set_defaults(f=cmd_plan)
    s = mit_wurzel(u.add_parser("export"))
    s.add_argument("-o", "--aus", required=True); s.set_defaults(f=cmd_export)
    s = mit_wurzel(u.add_parser("kernel")); s.add_argument("was")
    s.set_defaults(f=cmd_kernel)
    s = mit_wurzel(u.add_parser("source-add")); s.add_argument("url")
    s.add_argument("key"); s.set_defaults(f=cmd_source_add)
    s = mit_wurzel(u.add_parser("source-remove")); s.add_argument("url")
    s.set_defaults(f=cmd_source_remove)
    s = mit_wurzel(u.add_parser("set")); s.add_argument("key")
    s.add_argument("value"); s.set_defaults(f=cmd_set)
    s = mit_wurzel(u.add_parser("unset")); s.add_argument("key")
    s.set_defaults(f=cmd_unset)
    s = mit_wurzel(u.add_parser("settings")); s.set_defaults(f=cmd_settings)
    s = mit_wurzel(u.add_parser("account-add")); s.add_argument("name")
    s.add_argument("--uid", default="1000"); s.add_argument("--gid", default="1000")
    s.add_argument("--home", default=None)
    s.add_argument("--shell", default="/bin/sh")
    s.set_defaults(f=cmd_account_add)
    s = mit_wurzel(u.add_parser("account-remove")); s.add_argument("name")
    s.add_argument("--behalte-daten", dest="behalte_daten", action="store_true")
    s.set_defaults(f=cmd_account_remove)
    s = mit_wurzel(u.add_parser("secret-set")); s.add_argument("name")
    s.add_argument("datei"); s.set_defaults(f=cmd_secret_set)
    s = u.add_parser("asset-pack"); s.add_argument("datei")
    s.add_argument("-o", "--aus", required=True)
    s.add_argument("--class", dest="klasse", required=True,
                   choices=sorted(USER_BLOB_KEYS))
    s.add_argument("--name", default=None); s.set_defaults(f=cmd_asset_pack)
    s = mit_wurzel(u.add_parser("pref")); s.add_argument("user")
    s.add_argument("key"); s.add_argument("value"); s.set_defaults(f=cmd_pref)
    s = mit_wurzel(u.add_parser("pref-unset")); s.add_argument("user")
    s.add_argument("key"); s.set_defaults(f=cmd_pref_unset)
    s = mit_wurzel(u.add_parser("prefs")); s.add_argument("user", nargs="?")
    s.set_defaults(f=cmd_prefs)
    s = mit_wurzel(u.add_parser("snapshot")); s.set_defaults(f=cmd_snapshot)
    s = mit_wurzel(u.add_parser("verify")); s.set_defaults(f=cmd_verify)
    s = mit_wurzel(u.add_parser("rebuild"))
    s.add_argument("--plan", required=True)
    s.add_argument("--source", action="append")
    s.add_argument("--secrets", default=None)
    s.set_defaults(f=cmd_rebuild)

    a = p.parse_args(argv[1:])
    try:
        return a.f(a)
    except ValueError as e:
        print("opk: %s" % e, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
