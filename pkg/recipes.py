#!/usr/bin/env python3
"""pkg/recipes.py -- generate one recipe per userland program, by machine.

WHY THIS EXISTS. Before round PLAN2 exactly three of Osum's userland
programs had ever been packaged, and the reason was not that the rest are
hard: it was that every recipe had to be written by hand. A hand-written
recipe is a second place where a name lives, and a second place is always
the one that stops being true. `pkg/bauen.sh` already made this argument
for the five `.prog` bundles Osum ships -- this file makes it for
everything else.

WHAT IT READS, and nothing else:

  vendor/osum/bin/*            the built programs, from the PINNED commit
  vendor/osum/apps/*.prog      the bundles, which already have a recipe
  pkg/rezepte/*.rezept         the hand-written recipes
  vendor/osum/COMMIT           which Osum this is

WHAT IT DOES NOT DO: it does not copy the descriptions out of Osum's
source files. Those headers are half German and half English, and this
repository does not translate another project's prose by machine and then
ship the result as its own metadata. The generated `info` says what is
actually known -- the program's name and the commit it was built from.
A program that deserves a real description gets a bundle in Osum
(`assets/apps/<name>.prog/INFO`), and then `pkg/bauen.sh` reads THAT.

    ./pkg/recipes.py <output-directory>

Writes `<dir>/<name>.rezept` for every program that can be packaged and
prints one line per program -- `made` or `skipped <reason>` -- so the
count in the round log is a count and not an estimate.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)

# The words a package must not be called: they are the types of a typed
# PLAN line, and a package with such a name would make an old-shape plan
# line ambiguous. `pkg/opk.py` refuses them too -- this is the earlier of
# the two doors.
#
# THE LIST IS IMPORTED AND NOT COPIED. It used to be written out here as
# five names, and by the time round PLAN2 had added `pref` and `arch` the
# copy was wrong by two -- a package called `pref` would have been built
# here and refused three commands later. One list, in the program that
# owns the format.
sys.path.insert(0, HERE)
from opk import PLAN_TYPES as RESERVED          # noqa: E402

# The handles every packaged program gets: the three pots and the
# console. What an application may do stands in the package and not in
# its source (PACKAGING.md § 7).
#
# `konsole` IS GERMAN AND STAYS GERMAN HERE, on purpose. It is not a word
# this file invents -- it is the existing handle namespace, written that
# way in PACKAGING.md § 7 and in every package `pkg/bauen.sh` has ever
# built. Spelling it `console` in the generated recipes would put two
# names on one object, which is worse than one German name. It is listed
# as debt in docs/ROUND-PLAN2.md and belongs to the rename round.
HANDLES = ("cache", "config", "konsole", "state")


def bundle_starts(apps_dir):
    """Two maps: which /bin program a bundle starts, and the bundle NAMES.

    Both matter, and for different reasons. A program that a bundle
    already starts must not be packaged twice under a second name -- the
    same octets would sit in the store under two hashes. And a bundle
    NAME must not be taken by a different program: Osum's `suchen.prog`
    starts `/bin/starter`, while `/bin/suchen` is a different binary, so
    a package called `suchen` would mean two things. This is found here
    and named, rather than silently losing whichever came second.
    """
    starts, namen = {}, set()
    if not os.path.isdir(apps_dir):
        return starts, namen
    for d in sorted(os.listdir(apps_dir)):
        if not d.endswith(".prog"):
            continue
        namen.add(d[:-len(".prog")])
        p = os.path.join(apps_dir, d, "start.txt")
        if not os.path.exists(p):
            continue
        for zeile in open(p, encoding="utf-8", errors="replace"):
            zeile = zeile.strip()
            if zeile.startswith("/"):
                starts[os.path.basename(zeile)] = d[:-len(".prog")]
                break
    return starts, namen


def handmade(rezepte_dir):
    """Which programs a hand-written recipe already claims."""
    aus = {}
    if not os.path.isdir(rezepte_dir):
        return aus
    for r in sorted(os.listdir(rezepte_dir)):
        if not r.endswith(".rezept"):
            continue
        name = None
        for zeile in open(os.path.join(rezepte_dir, r), encoding="utf-8"):
            z = zeile.split("#")[0].strip()
            if z.startswith("name="):
                name = z[len("name="):].strip()
            elif z.startswith("datei=start "):
                aus[os.path.basename(z.split()[-1])] = name or r
    return aus


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    aus_dir = argv[1]
    os.makedirs(aus_dir, exist_ok=True)

    bin_dir = os.path.join(REPO, "vendor", "osum", "bin")
    apps_dir = os.path.join(REPO, "vendor", "osum", "apps")
    rez_dir = os.path.join(REPO, "pkg", "rezepte")
    commit_f = os.path.join(REPO, "vendor", "osum", "COMMIT")
    if not os.path.isdir(bin_dir):
        print("vendor/osum/bin is not there -- "
              "./vendor/osum/hole-osum.sh first", file=sys.stderr)
        return 1
    commit = open(commit_f).read().strip() if os.path.exists(commit_f) else "?"

    schon_bundle, bundle_namen = bundle_starts(apps_dir)
    schon_hand = handmade(rez_dir)

    programme = sorted(n for n in os.listdir(bin_dir)
                       if os.path.isfile(os.path.join(bin_dir, n)))
    gemacht, uebersprungen = [], []
    for name in programme:
        if name in schon_bundle:
            uebersprungen.append((name, "bundle %s.prog already packages it"
                                  % schon_bundle[name]))
            continue
        if name in schon_hand:
            uebersprungen.append((name, "hand-written recipe %s packages it"
                                  % schon_hand[name]))
            continue
        if name in RESERVED:
            uebersprungen.append((name, "the name is a PLAN type"))
            continue
        if name in bundle_namen:
            uebersprungen.append(
                (name, "the bundle %s.prog already owns that package name "
                       "but starts a different program -- packaging this "
                       "binary needs a second name, which is a decision and "
                       "not a default" % name))
            continue
        pfad = os.path.join(aus_dir, name + ".rezept")
        with open(pfad, "w", encoding="utf-8") as f:
            f.write("# Generated by pkg/recipes.py from vendor/osum/bin/%s.\n"
                    "# Do not edit -- the next build overwrites it.\n" % name)
            f.write("name=%s\n" % name)
            f.write("fassung=1.0.0\n")
            f.write("titel=%s\n" % name)
            f.write("info=userland program %s, built from Osum %s\n"
                    % (name, commit[:8]))
            f.write("keys=%s\n" % name)
            for h in HANDLES:
                f.write("handle=%s\n" % h)
            f.write("origin=osum:%s\n" % commit)
            f.write("datei=start %s\n" % os.path.join(REPO, "vendor", "osum",
                                                      "bin", name))
        gemacht.append(name)
        print("made    %s" % name)
    for name, warum in uebersprungen:
        print("skipped %-12s %s" % (name, warum))
    print("%d program(s) in vendor/osum/bin, %d recipe(s) written, "
          "%d skipped" % (len(programme), len(gemacht), len(uebersprungen)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
