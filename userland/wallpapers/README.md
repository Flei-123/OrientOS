# `userland/wallpapers/` — the backgrounds, as text

A wallpaper is an image, and an image cannot go into a `PLAN` line. What
goes into the plan is its SHA-256; the octets become an **asset package**
in the store (`kind=asset`, `class=wallpaper`).

The octets themselves are still generated from **text**, for the same
reason Osum's `tools/k15/symbol.py` draws icons from text: a blob checked
into a repository is something nobody can read in a diff. A file here is
four lines, `pkg/osym.py` turns it into the OSYM image
`kernel/user/schreibtisch.fi` displays, and the same four lines always
produce the same octets — so the hash in a plan is reproducible from the
repository and not only from a copy of the image.

The size limit is not ours: a process has 832 KiB of heap in this system
and the desktop stretches the source, so `schreibtisch.fi` caps it at
240 x 180 (`BILD_MAX_W`, `BILD_MAX_H`). `pkg/osym.py` refuses anything
larger instead of producing an image that cannot be loaded.
