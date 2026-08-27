#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""pkg/osym.py -- turn a four-line text file into an OSYM image.

WHY THIS EXISTS. A wallpaper is content, and content in a repository
should be readable in a diff. Osum makes the same argument for program
icons (`tools/k15/symbol.py`: "a symbol that lies in the tree as a lump
of octets is something nobody can read in a difference"), and it holds
harder for something 240 x 180: nobody reviews 172 800 octets.

So the source is text, and the octets are generated. The point that
matters for a PLAN is that the generation is DETERMINISTIC -- the same
four lines always produce the same image, so the SHA-256 a plan carries
can be reproduced from this repository and not only from a copy of the
image that happens to still exist somewhere.

THE FORMAT IT WRITES is Osum's OSYM, twelve octets of header
(`kernel/user/schreibtisch.fi`, `tools/k15/symbol.py`):

    +0   "OSYM"
    +4   width    u32, little endian
    +8   height   u32, little endian
    +12  width * height pixels of four octets: B, G, R, A

THE SIZE LIMIT IS NOT OURS. A process has 832 KiB of heap in this
system, 253 of which the widget canvas already holds, so the desktop
caps its source image at 240 x 180 and stretches it to the screen
(`BILD_MAX_W`, `BILD_MAX_H`). Anything larger is refused here rather
than produced and then silently not loaded.

THE SOURCE:

    width=240
    height=180
    top=1d2b3a        the colour of the first row, rrggbb
    bottom=090d12     the colour of the last row

Between them the rows are interpolated linearly and rounded the same way
every time. A single flat colour is `top` equal to `bottom`.

    osym.py <file.wallpaper> <out.osym>
    osym.py --show <out.osym>       header and a few pixels, for checking
"""

import struct
import sys

MAX_W, MAX_H = 240, 180


def read_source(path):
    felder = {}
    for nr, roh in enumerate(open(path, encoding="ascii").read().splitlines(), 1):
        z = roh.split("#")[0].strip()
        if not z:
            continue
        if "=" not in z:
            raise SystemExit("osym: %s line %d: '%s' is not key=value"
                             % (path, nr, z))
        k, _, v = z.partition("=")
        felder[k.strip()] = v.strip()
    for k in ("width", "height", "top", "bottom"):
        if k not in felder:
            raise SystemExit("osym: %s has no '%s'" % (path, k))
    return felder


def colour(text, where):
    if len(text) != 6 or any(c not in "0123456789abcdefABCDEF" for c in text):
        raise SystemExit("osym: %s is not six hex digits (%s)" % (where, text))
    v = int(text, 16)
    return (v >> 16) & 255, (v >> 8) & 255, v & 255


def build(felder, where):
    w, h = int(felder["width"]), int(felder["height"])
    if not 1 <= w <= MAX_W or not 1 <= h <= MAX_H:
        raise SystemExit(
            "osym: %s is %dx%d; the desktop of this system loads at most "
            "%dx%d (schreibtisch.fi, BILD_MAX_W/H) and stretches it"
            % (where, w, h, MAX_W, MAX_H))
    r0, g0, b0 = colour(felder["top"], "top")
    r1, g1, b1 = colour(felder["bottom"], "bottom")
    aus = bytearray(b"OSYM" + struct.pack("<II", w, h))
    teiler = (h - 1) or 1
    for y in range(h):
        # Integer arithmetic on purpose: rounding must not depend on a
        # floating point mode anywhere.
        r = r0 + (r1 - r0) * y // teiler
        g = g0 + (g1 - g0) * y // teiler
        b = b0 + (b1 - b0) * y // teiler
        aus += bytes((b, g, r, 255)) * w
    return bytes(aus)


def show(path):
    roh = open(path, "rb").read()
    if roh[:4] != b"OSYM":
        raise SystemExit("osym: %s does not start with OSYM" % path)
    w, h = struct.unpack_from("<II", roh, 4)
    print("%s: %dx%d, %d octet(s), header 12 + %d pixels"
          % (path, w, h, len(roh), w * h))
    for y in (0, h // 2, h - 1):
        off = 12 + (y * w) * 4
        b, g, r, a = roh[off:off + 4]
        print("   row %-4d first pixel  r=%02x g=%02x b=%02x a=%02x"
              % (y, r, g, b, a))
    return 0


def main(argv):
    if len(argv) == 3 and argv[1] == "--show":
        return show(argv[2])
    if len(argv) != 3:
        print(__doc__)
        return 2
    roh = build(read_source(argv[1]), argv[1])
    with open(argv[2], "wb") as f:
        f.write(roh)
    print("%s -> %s, %d octet(s)" % (argv[1], argv[2], len(roh)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
