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
# Die Wurzel: store, apps, users, system
# ---------------------------------------------------------------------------
class Wurzel:
    def __init__(self, pfad):
        self.p = os.path.abspath(pfad)
        self.store = os.path.join(self.p, "store")
        self.apps = os.path.join(self.p, "apps")
        self.users = os.path.join(self.p, "users")
        self.gen = os.path.join(self.p, "system", "generations")

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
        f = os.path.join(self.gen, str(n), "PLAN")
        if not os.path.exists(f):
            return {}
        aus = {}
        for z in open(f):
            z = z.rstrip("\n")
            if z:
                name, h = z.split("\t")
                aus[name] = h
        return aus

    def plan_schreiben(self, n, plan, grund):
        d = os.path.join(self.gen, str(n))
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "PLAN"), "w") as f:
            for name in sorted(plan):
                f.write("%s\t%s\n" % (name, plan[name]))
        with open(os.path.join(d, "GRUND"), "w") as f:
            f.write(grund + "\n")

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
        plan = self.plan(n)
        if os.path.isdir(self.apps):
            shutil.rmtree(self.apps)
        os.makedirs(self.apps, exist_ok=True)
        for name in sorted(plan):
            h = plan[name]
            quelle = os.path.join(self.store, kurz(h))
            if not os.path.isdir(quelle):
                raise SystemExit("opk: Generation %d nennt %s (%s), "
                                 "aber der Store hat den Eintrag nicht"
                                 % (n, name, h[:12]))
            self._verweisen(quelle, os.path.join(self.apps, name + ".prog"))
        self.aktuell_setzen(n)

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
            aus |= set(self.plan(n).values())
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

    plan = dict(w.plan(w.aktuell()))
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
    n = w.neue_generation(plan, "installiert %s %s"
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
    plan = dict(w.plan(w.aktuell()))
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
    n = w.neue_generation(plan, "entfernt %s" % args.was)
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
        plan = w.plan(n)
        grund = open(os.path.join(w.gen, str(n), "GRUND")).read().strip()
        print("%s %3d  %2d Paket(e)  %s"
              % ("*" if n == a else " ", n, len(plan), grund))
    return 0


def zurueck(args):
    w = Wurzel(args.wurzel)
    n = int(args.n)
    if n not in w.generationen():
        raise SystemExit("opk: Generation %d gibt es nicht" % n)
    vorher = w.aktuell()
    w.aktivieren(n)
    plan = w.plan(n)
    print("zurueck auf Generation %d (war %d): %d Paket(e) (%s)"
          % (n, vorher, len(plan), ", ".join(sorted(plan)) or "keines"))
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


def main(argv):
    p = argparse.ArgumentParser(
        prog="opk.py", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    u = p.add_subparsers(dest="befehl", required=True)

    def mit_wurzel(sp):
        sp.add_argument("--wurzel", required=True)
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

    a = p.parse_args(argv[1:])
    try:
        return a.f(a)
    except ValueError as e:
        print("opk: %s" % e, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
