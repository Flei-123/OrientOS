# tests/step-70-doku.sh — wird von test.sh gesourct, nicht direkt gestartet.
#
# DAMIT DIE DOKU NICHT LEISE VERROTTET.
#
# Es gab in diesem Projekt zwei Kernel mit demselben Namen, und der Grund
# war nicht Absicht, sondern Doku, die stehengeblieben ist: LANGUAGE.md
# beschrieb eine modulweise Migration nach Firn, waehrend daneben ein
# fertiger Firn-Kernel entstand. Zwei Wahrheiten in einem Projekt sind
# eine Wahrheit zu viel.
#
# Dieser Schritt prueft deshalb nicht Code, sondern ob die Dokumente noch
# dasselbe sagen wie der Baum:
#
#   1. KERNELWECHSEL.md enthaelt wirklich den Abgleich, das Portierte,
#      das Offene und die Geschichte — nicht nur Ueberschriften.
#   2. Der alte Migrationsstand wird nirgends mehr als GELTEND behauptet,
#      und die Zahl "noch in Rust" stimmt mit dem Baum ueberein.
#   3. JEDER PFAD, den ein Dokument nennt, existiert. Das ist der Ersatz
#      fuer die alte Zeilennummernpruefung (tests/verweise.py): die
#      zeigte auf Rust-Dateien, die es nicht mehr gibt. Ein Verweis auf
#      eine Datei, die es nicht gibt, ist schlimmer als keiner — er sieht
#      ueberpruefbar aus und ist es nicht.
#   4. Was noch offen ist, wird auch als offen benannt, und was geloescht
#      wurde, steht als geloescht da. Nichts wird wegretuschiert.
#   5. GEGENPROBE: die Pfadpruefung muss bei einem erfundenen Pfad
#      wirklich anschlagen.
step "Doku: die Dokumente sagen dasselbe wie der Baum"
doku_check() {
    RC=0
    local abschnitt d

    # --- 1. KERNELWECHSEL.md
    if [[ ! -f KERNELWECHSEL.md ]]; then
        nok "KERNELWECHSEL.md fehlt"; return 1
    fi
    for abschnitt in \
        '## 2. Der Abgleich, Modul für Modul' \
        '## 3. Was portiert wurde' \
        '## 4. Was NOCH offen ist' \
        '## 6. Die Geschichte dieses Wechsels' \
        '## 7. Der Schnitt'
    do
        grep -qF "$abschnitt" KERNELWECHSEL.md \
            && ok "KERNELWECHSEL.md: $abschnitt" \
            || nok "KERNELWECHSEL.md: Abschnitt fehlt — $abschnitt"
    done
    local zeilen
    zeilen=$(grep -cE '^\| .* \| .* \| .* \|' KERNELWECHSEL.md)
    [[ "$zeilen" -ge 20 ]] \
        && ok "der Abgleich hat $zeilen Tabellenzeilen" \
        || nok "der Abgleich hat nur $zeilen Tabellenzeilen — das ist keine Modul-fuer-Modul-Pruefung"

    # --- 2. Alle Hauptdokumente verweisen darauf
    for d in README.md ARCHITECTURE.md ROADMAP.md LANGUAGE.md; do
        grep -qF 'KERNELWECHSEL.md' "$d" \
            && ok "$d verweist auf KERNELWECHSEL.md" \
            || nok "$d verweist nicht auf KERNELWECHSEL.md — dort steht, was gilt"
    done

    # --- 3. Der alte Stand wird nicht mehr als geltend behauptet
    grep -qF 'ÜBERHOLT AM 25.08.2026' LANGUAGE.md \
        && ok "LANGUAGE.md kennzeichnet den alten Migrationsstand als überholt" \
        || nok "LANGUAGE.md behauptet den alten Migrationsstand weiter als geltend"
    # Die Zahl aus M-00, die nach dem Schnitt schlicht falsch ist.
    if grep -qF '17 993 Zeilen' LANGUAGE.md && ! grep -qF 'Geschichte' LANGUAGE.md; then
        nok "LANGUAGE.md nennt 17 993 Zeilen Rust, ohne sie als Geschichte zu kennzeichnen"
    else
        ok "die alte Zahl in LANGUAGE.md steht als Geschichte da, nicht als Stand"
    fi
    grep -qF 'Der Umbau läuft modulweise' README.md \
        && nok "README.md kuendigt weiter die modulweise Migration DIESES Baums an" \
        || ok "README.md kuendigt keine modulweise Migration dieses Baums mehr an"

    # --- 4. Die Zahl stimmt mit dem Baum
    local ist
    ist=$(git ls-files '*.rs' | grep -v '^vorlage/' | xargs cat 2>/dev/null | wc -l)
    if [[ "$ist" -eq 0 ]]; then
        ok "im Baum stehen 0 Zeilen Rust ausser der Vorlage"
    else
        nok "im Baum stehen $ist Zeilen Rust ausserhalb von vorlage/"
    fi
    local vorlage_zeilen
    vorlage_zeilen=$(wc -l < vorlage/arch_iface.rs)
    if grep -qF "$vorlage_zeilen Zeilen" KERNELWECHSEL.md; then
        ok "KERNELWECHSEL.md nennt die Groesse der Vorlage richtig ($vorlage_zeilen Zeilen)"
    else
        nok "KERNELWECHSEL.md nennt nicht $vorlage_zeilen Zeilen fuer vorlage/arch_iface.rs"
    fi

    # --- 5. Jeder genannte Pfad existiert
    local fehlend
    fehlend=$(python3 - <<'PY'
import re, pathlib
# WELCHE PFADE GEPRUEFT WERDEN, und warum nicht alle.
#
# Die Dokumente dieses Repos nennen aus gutem Grund Pfade, die es HIER
# nicht gibt: die des Osum-Repos (`kernel/…`, `lib/…`, `tools/…`,
# `docs/…`) liegen in einem anderen Repository, und blosse DATEINAMEN
# ohne Verzeichnis (`Cargo.toml`, `mkfs.py`, `acpi.fi`, `osum.sh` — das
# ist in NAMEN.md eine Domain!) sind Namen, keine Pfade.
#
# `docs/` steht ABSICHTLICH NICHT in dieser Liste, obwohl dieses Repo seit
# dem 26.08.2026 selbst eines hat: BEIDE Repos haben ein `docs/`, und die
# Dokumente hier verweisen mit gutem Grund auf Osums Rundenlogbuecher
# (`docs/ROUNDK15.md` und andere), die dort und nicht hier liegen. Ein
# Praefix, das in zwei Repos etwas anderes bedeutet, taugt nicht als
# Unterscheidungsmerkmal -- und ein Test, der deshalb falsch anschlaegt,
# waere schlimmer als keiner.
#
# Geprueft wird deshalb genau das, was ein Leser HIER anklicken koennen
# muss: ein Pfad, dessen erster Bestandteil ein Verzeichnis dieses Repos
# ist, oder eine Datei im Wurzelverzeichnis mit .md-Endung.
#
# UND: ein solcher Pfad darf fehlen, WENN er in tests/GELOESCHT.md steht.
# Das ist keine Ausnahme, sondern der Punkt — wer etwas loescht, sagt
# dort, wo es jetzt steht. Wer das vergisst, faellt hier durch.
lokal = {'tests', 'userland', 'brands', 'vendor', 'vorlage', 'assets', 'pkg'}
muster = re.compile(r'`([A-Za-z0-9_./-]+\.(?:md|sh|py|toml|fi|rs|conf|json|ld|asm|img|txt))`')
geloescht = pathlib.Path('tests/GELOESCHT.md').read_text(encoding='utf-8')
fehlt = set()
for md in sorted(pathlib.Path('.').glob('*.md')) + [pathlib.Path('tests/README.md'),
                                                    pathlib.Path('userland/README.md')]:
    if md.name == 'GAUNTLET.md' or not md.exists():
        continue          # GAUNTLET.md: automatisch erzeugtes Protokoll
    for m in muster.finditer(md.read_text(encoding='utf-8', errors='replace')):
        p = m.group(1)
        teile = p.split('/')
        if len(teile) == 1:
            if not p.endswith('.md'):
                continue
        elif teile[0] not in lokal:
            continue
        if pathlib.Path(p).exists():
            continue
        if f'`{p}`' in geloescht:
            continue
        fehlt.add(f'{md.name}: {p}')
for z in sorted(fehlt):
    print(z)
PY
)
    if [[ -z "$fehlend" ]]; then
        ok "jeder Pfad, den die Dokumente nennen, existiert wirklich"
    else
        nok "die Dokumente nennen Pfade, die es nicht gibt:"
        echo "$fehlend" | sed 's/^/         /'
    fi
    # GEGENPROBE: die Pruefung muss anschlagen.
    local tmp
    tmp=$(mktemp -d)
    mkdir -p "$tmp/tests"
    printf 'Probe: `tests/gibtsnicht.sh`\n' > "$tmp/PROBE.md"
    : > "$tmp/tests/GELOESCHT.md"
    if (cd "$tmp" && python3 - <<'PY' | grep -q .
import re, pathlib
lokal = {'tests', 'userland', 'brands', 'vendor', 'vorlage', 'assets', 'pkg'}
muster = re.compile(r'`([A-Za-z0-9_./-]+\.(?:md|sh|py|toml|fi|rs|conf|json|ld|asm|img|txt))`')
geloescht = pathlib.Path('tests/GELOESCHT.md').read_text()
for md in sorted(pathlib.Path('.').glob('*.md')):
    for m in muster.finditer(md.read_text()):
        p = m.group(1)
        t = p.split('/')
        if len(t) == 1 and not p.endswith('.md'):
            continue
        if len(t) > 1 and t[0] not in lokal:
            continue
        if not pathlib.Path(p).exists() and f'`{p}`' not in geloescht:
            print(p)
PY
    ); then
        ok "Gegenprobe: ein erfundener Pfad laesst die Pruefung anschlagen"
    else
        nok "die Pfadpruefung schlaegt bei einem erfundenen Pfad NICHT an"
    fi
    rm -rf "$tmp"

    # --- 6. Was offen ist, steht als offen da
    grep -qF 'vorlage/arch_iface.rs' KERNELWECHSEL.md \
        && ok "der offene Punkt 'arch-Grenze' nennt die Vorlage, die ihn belegt" \
        || nok "KERNELWECHSEL.md § 4 nennt die Vorlage nicht"
    grep -qiE 'NotSupported|Kanäle, Ports' KERNELWECHSEL.md \
        && ok "die nicht portierten Objekte der nativen ABI stehen als offen da" \
        || nok "KERNELWECHSEL.md sagt nicht, dass Kanaele/Ports/Namensraeume fehlen"
    # ...und was NICHT mehr offen ist, steht auch nicht mehr als offen da.
    if grep -qE '^\| .*SMEP/SMAP.*OFFEN' KERNELWECHSEL.md; then
        nok "SMEP/SMAP steht noch als offen — es ist portiert (Osum, kernel/guard.fi)"
    else
        ok "SMEP/SMAP steht nicht mehr als offen"
    fi
    if grep -qE 'Rahmenpufferkonsole.*OFFEN|fbcon.*\*\*OFFEN' KERNELWECHSEL.md; then
        nok "die Rahmenpufferkonsole steht noch als offen — Osum hat seit K7 einen Bildschirm"
    else
        ok "die Rahmenpufferkonsole steht nicht mehr als offen"
    fi

    # --- 7. Was geloescht wurde, steht als geloescht da
    for d in README.md ARCHITECTURE.md ROADMAP.md LANGUAGE.md KERNELWECHSEL.md; do
        if grep -qE 'kernel/src|libs/osum-abi|run-qemu\.sh|cargo' "$d" \
           && ! grep -qiE 'gelöscht|geloescht|Historie|Geschichte|bis zum' "$d"; then
            nok "$d nennt geloeschte Dinge, ohne zu sagen, dass sie geloescht sind"
        else
            ok "$d nennt Geloeschtes nur als Geschichte"
        fi
    done

    # --- 8. userland/ sagt, was daraus geworden ist
    if grep -qF 'userland/PROGRAMME' userland/README.md \
       && grep -qiE 'gelöscht|geloescht' userland/README.md \
       && ! grep -qE '^\| .hello\.asm. \|' userland/README.md; then
        ok "userland/README.md beschreibt die neue Rolle und nennt hello.asm nur als Geschichte"
    else
        nok "userland/README.md beschreibt noch das geloeschte Startdateisystem"
    fi
    return $RC
}
run doku_check
