/* tests/firn-bitmap/pruefstand.c -- stellt kernel/firn/bitmap.fi genau die
 * Fragen, die libs/osum-mem/src/bitmap.rs in seinen 23 Testfaellen beantwortet.
 *
 * WARUM IN C UND NICHT IN RUST
 *
 * Das Firn-Objekt wird im Kernelprofil uebersetzt: freistehend, ohne libc,
 * ohne Laufzeit. Ein Rust-Test wuerde es in eine Crate zwingen, die fuer den
 * Host gebaut wird -- dann pruefte man ein anderes Objekt als das, was spaeter
 * im Kernel landet. Hier wird GENAU DIESES Objekt gelinkt und aufgerufen.
 *
 * WARUM DAS UEBERHAUPT NOETIG IST
 *
 * Die Rust-Fassung hat 23 Tests, darunter einen Eigenschaftstest gegen ein
 * Referenzmodell. Diese Tests sind der eigentliche Wert der Datei -- ohne sie
 * waere ein Verwalter physischen Speichers nicht vertrauenswuerdig. Eine
 * Neufassung in einer anderen Sprache muss dieselben Fragen bestehen, sonst
 * ist "portiert" nur eine Behauptung.
 *
 * Der Zufallsgenerator ist Zeile fuer Zeile derselbe wie
 * libs/osum-mem/src/testrand.rs (LCG mit MMIX-Konstanten, Wortrotation).
 * Beide Fassungen sehen damit die IDENTISCHE Folge von Anforderungen.
 *
 * Uebersetzen und laufen lassen: tests/firn-bitmap/lauf.sh
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

/* Der Verwaltungsblock -- Feld fuer Feld wie `struct Bitmap` in bitmap.fi. */
typedef struct {
    uint64_t bits;
    uint64_t frames;
    uint64_t used;
    uint64_t cursor;
} Bitmap;

#define KEIN_RAHMEN 0xFFFFFFFFFFFFFFFFULL
#define E_OK 0
#define E_OUT_OF_RANGE 2

/* Die Firn-Seite. `#[export_c]` heisst: der Name steht unveraendert im
 * Objekt, es braucht keine Namensuebersetzung. */
extern uint64_t bm_words_needed(uint64_t frames);
extern bool     bm_init(Bitmap *bm, uint64_t bits, uint64_t woerter, uint64_t frames);
extern uint64_t bm_frames(Bitmap *bm);
extern uint64_t bm_used_frames(Bitmap *bm);
extern uint64_t bm_free_frames(Bitmap *bm);
extern bool     bm_is_used(Bitmap *bm, uint64_t i);
extern void     bm_free_range(Bitmap *bm, uint64_t start, uint64_t ende);
extern void     bm_reserve_range(Bitmap *bm, uint64_t start, uint64_t ende);
extern uint64_t bm_alloc(Bitmap *bm);
extern uint64_t bm_alloc_contiguous(Bitmap *bm, uint64_t anzahl);
extern uint64_t bm_free(Bitmap *bm, uint64_t index, uint64_t anzahl);

/* Firns geprüfte Arithmetik landet hier. Im Kernel schreibt osum eine Meldung
 * und haelt an; im Pruefstand ist jeder Anschlag ein FEHLGESCHLAGENER TEST --
 * die Bitmap darf bei keiner Eingabe ueberlaufen, auch nicht bei UINT64_MAX. */
static int panik_gezaehlt = 0;

void osum_panic(const char *msg, uint32_t len, uint64_t a, uint64_t b, uint64_t art)
{
    panik_gezaehlt++;
    printf("\n  [FEHL] Firns Pruefung schlug an, wo sie es nicht darf:\n");
    printf("         %.*s\n", (int)len, msg);
    printf("         a=%llu b=%llu art=%llu\n",
           (unsigned long long)a, (unsigned long long)b, (unsigned long long)art);
    exit(1);
}

/* ------------------------------------------------------------ Testgeruest */

static int bestanden = 0;
static int gefallen = 0;
static const char *laufender_test = "";

#define PRUEFE(bed)                                                            \
    do {                                                                       \
        if (!(bed)) {                                                          \
            printf("  [FEHL] %s -- Zeile %d: %s\n", laufender_test, __LINE__,  \
                   #bed);                                                      \
            gefallen++;                                                        \
            return;                                                            \
        }                                                                      \
    } while (0)

#define GLEICH(a, b)                                                           \
    do {                                                                       \
        unsigned long long va = (unsigned long long)(a);                       \
        unsigned long long vb = (unsigned long long)(b);                       \
        if (va != vb) {                                                        \
            printf("  [FEHL] %s -- Zeile %d: %s = %llu, erwartet %llu\n",      \
                   laufender_test, __LINE__, #a, va, vb);                      \
            gefallen++;                                                        \
            return;                                                            \
        }                                                                      \
    } while (0)

#define TEST(name)                                                             \
    static void name(void);                                                    \
    static void lauf_##name(void) {                                            \
        laufender_test = #name;                                                \
        int vorher = gefallen;                                                 \
        name();                                                                \
        if (gefallen == vorher) {                                              \
            bestanden++;                                                       \
            printf("  [ ok ] %s\n", #name);                                    \
        }                                                                      \
    }                                                                          \
    static void name(void)

/* Ein Testaufbau: Bitspeicher plus Verwaltungsblock, alles auf dem Stapel des
 * Aufrufers -- genau wie im Kernel, wo beides vor dem Heap existiert. */
typedef struct {
    Bitmap bm;
    uint64_t worte[64];
} Aufbau;

static void aufbau(Aufbau *a, uint64_t frames, uint64_t woerter)
{
    memset(a->worte, 0, sizeof(a->worte));
    bool ok = bm_init(&a->bm, (uint64_t)(uintptr_t)a->worte, woerter, frames);
    if (!ok) {
        printf("  [FEHL] %s: bm_init abgewiesen (frames=%llu woerter=%llu)\n",
               laufender_test, (unsigned long long)frames,
               (unsigned long long)woerter);
        exit(1);
    }
}

/* ------------------------------------------------- die Faelle aus bitmap.rs */

TEST(alles_zunaechst_belegt)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    GLEICH(bm_free_frames(&a.bm), 0);
    PRUEFE(bm_is_used(&a.bm, 0));
}

TEST(alloc_und_free_hin_und_zurueck)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    GLEICH(bm_free_frames(&a.bm), 256);
    uint64_t f = bm_alloc(&a.bm);
    GLEICH(f, 0);
    PRUEFE(bm_is_used(&a.bm, 0));
    GLEICH(bm_free_frames(&a.bm), 255);
    GLEICH(bm_free(&a.bm, f, 1), E_OK);
    GLEICH(bm_free_frames(&a.bm), 256);
}

TEST(zusammenhaengend_ueberspringt_belegtes)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    bm_reserve_range(&a.bm, 0, 10);
    GLEICH(bm_alloc_contiguous(&a.bm, 8), 10);
    GLEICH(bm_free_frames(&a.bm), 256 - 10 - 8);
}

TEST(erschoepfung_meldet_oom)
{
    Aufbau a;
    aufbau(&a, 64, 1);
    bm_free_range(&a.bm, 0, 64);
    for (int i = 0; i < 64; i++) {
        PRUEFE(bm_alloc(&a.bm) != KEIN_RAHMEN);
    }
    GLEICH(bm_alloc(&a.bm), KEIN_RAHMEN);
}

TEST(words_needed_rundet_auf)
{
    GLEICH(bm_words_needed(1), 1);
    GLEICH(bm_words_needed(64), 1);
    GLEICH(bm_words_needed(65), 2);
    GLEICH(bm_words_needed(0), 0);
    GLEICH(bm_words_needed(128), 2);
    GLEICH(bm_words_needed(129), 3);
}

TEST(rahmenzahl_muss_kein_vielfaches_von_64_sein)
{
    /* 70 Rahmen in 2 Woertern: die 58 ueberzaehligen Bits duerfen NIE
     * vergeben werden, sonst zeigt ein Rahmen auf Speicher, den es nicht gibt. */
    Aufbau a;
    aufbau(&a, 70, 2);
    bm_free_range(&a.bm, 0, 1000); /* absichtlich ueber das Ende hinaus */
    GLEICH(bm_free_frames(&a.bm), 70);
    int gesehen = 0;
    uint64_t f;
    while ((f = bm_alloc(&a.bm)) != KEIN_RAHMEN) {
        PRUEFE(f < 70);
        gesehen++;
    }
    GLEICH(gesehen, 70);
    GLEICH(bm_free_frames(&a.bm), 0);
    PRUEFE(bm_is_used(&a.bm, 70));
    PRUEFE(bm_is_used(&a.bm, UINT64_MAX));
}

TEST(reserve_und_free_sind_idempotent)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    bm_free_range(&a.bm, 0, 256);
    GLEICH(bm_free_frames(&a.bm), 256);
    bm_reserve_range(&a.bm, 10, 20);
    bm_reserve_range(&a.bm, 10, 20);
    GLEICH(bm_free_frames(&a.bm), 246);
    bm_free_range(&a.bm, 10, 20);
    GLEICH(bm_free_frames(&a.bm), 256);
    GLEICH(bm_used_frames(&a.bm), 0);
    GLEICH(bm_frames(&a.bm), 256);
}

TEST(leere_und_verdrehte_bereiche_tun_nichts)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    bm_free_range(&a.bm, 100, 100);
    bm_reserve_range(&a.bm, 100, 100);
    bm_reserve_range(&a.bm, 200, 50); /* ende < start */
    GLEICH(bm_free_frames(&a.bm), 256);
}

TEST(alloc_weist_unmoegliche_anzahlen_ab)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    GLEICH(bm_alloc_contiguous(&a.bm, 0), KEIN_RAHMEN);
    GLEICH(bm_alloc_contiguous(&a.bm, 257), KEIN_RAHMEN);
    GLEICH(bm_free_frames(&a.bm), 256);
    GLEICH(bm_alloc_contiguous(&a.bm, 256), 0);
    GLEICH(bm_free_frames(&a.bm), 0);
}

TEST(free_ausserhalb_wird_abgewiesen)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    uint64_t f = bm_alloc_contiguous(&a.bm, 4);
    GLEICH(bm_free(&a.bm, 250, 10), E_OUT_OF_RANGE);
    GLEICH(bm_free(&a.bm, 256, 1), E_OUT_OF_RANGE);
    GLEICH(bm_free_frames(&a.bm), 252);
    GLEICH(bm_free(&a.bm, f, 4), E_OK);
    GLEICH(bm_free_frames(&a.bm), 256);
    GLEICH(bm_free(&a.bm, f, 4), E_OK);
    GLEICH(bm_free_frames(&a.bm), 256);
}

TEST(allokation_ueber_wortgrenzen)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    bm_reserve_range(&a.bm, 0, 60);
    GLEICH(bm_alloc_contiguous(&a.bm, 10), 60);
    for (uint64_t i = 60; i < 70; i++) {
        PRUEFE(bm_is_used(&a.bm, i));
    }
    PRUEFE(!bm_is_used(&a.bm, 70));
}

TEST(suchzeiger_laeuft_um_und_nutzt_freigegebenes)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    uint64_t alle[256];
    for (int i = 0; i < 256; i++) {
        alle[i] = bm_alloc(&a.bm);
        PRUEFE(alle[i] != KEIN_RAHMEN);
    }
    GLEICH(bm_alloc(&a.bm), KEIN_RAHMEN);
    GLEICH(bm_free(&a.bm, 128, 1), E_OK);
    GLEICH(bm_alloc(&a.bm), 128);
    GLEICH(bm_free(&a.bm, 0, 1), E_OK);
    GLEICH(bm_alloc(&a.bm), 0);
    GLEICH(bm_alloc(&a.bm), KEIN_RAHMEN);
    for (int i = 0; i < 256; i++) {
        GLEICH(bm_free(&a.bm, alle[i], 1), E_OK);
    }
    GLEICH(bm_free_frames(&a.bm), 256);
}

TEST(zerstueckelung_blockt_grosse_anforderungen)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    for (uint64_t i = 0; i < 256; i += 2) {
        bm_reserve_range(&a.bm, i, i + 1);
    }
    GLEICH(bm_free_frames(&a.bm), 128);
    GLEICH(bm_alloc_contiguous(&a.bm, 2), KEIN_RAHMEN);
    PRUEFE(bm_alloc_contiguous(&a.bm, 1) != KEIN_RAHMEN);
}

TEST(bitmap_mit_einem_einzigen_rahmen)
{
    Aufbau a;
    aufbau(&a, 1, 1);
    bm_free_range(&a.bm, 0, 1);
    GLEICH(bm_alloc(&a.bm), 0);
    GLEICH(bm_alloc(&a.bm), KEIN_RAHMEN);
    GLEICH(bm_free(&a.bm, 0, 1), E_OK);
    GLEICH(bm_alloc_contiguous(&a.bm, 1), 0);
}

TEST(bitmap_ohne_rahmen_vergibt_nie)
{
    /* Entartete Memory-Map: kein einziger nutzbarer Rahmen. */
    Aufbau a;
    aufbau(&a, 0, 1);
    GLEICH(bm_frames(&a.bm), 0);
    GLEICH(bm_free_frames(&a.bm), 0);
    GLEICH(bm_alloc(&a.bm), KEIN_RAHMEN);
    GLEICH(bm_alloc_contiguous(&a.bm, 1), KEIN_RAHMEN);
    PRUEFE(bm_is_used(&a.bm, 0));
    bm_free_range(&a.bm, 0, 10); /* darf nicht abstuerzen */
    GLEICH(bm_free_frames(&a.bm), 0);
}

TEST(zu_kleiner_bitspeicher_wird_abgewiesen)
{
    /* Die Rust-Fassung bricht hier mit assert! ab; die Firn-Fassung liefert
     * false, damit der fruehe Boot noch reagieren kann. */
    Aufbau a;
    memset(a.worte, 0, sizeof(a.worte));
    bool ok = bm_init(&a.bm, (uint64_t)(uintptr_t)a.worte, 1, 65);
    PRUEFE(!ok);
}

TEST(allokationen_ueberlappen_nie)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    bool gehoert[256];
    memset(gehoert, 0, sizeof(gehoert));
    uint64_t runde = 0;
    for (;;) {
        uint64_t n = 1 + runde % 3;
        runde++;
        uint64_t f = bm_alloc_contiguous(&a.bm, n);
        if (f == KEIN_RAHMEN) {
            break;
        }
        for (uint64_t i = f; i < f + n; i++) {
            PRUEFE(!gehoert[i]);
            gehoert[i] = true;
        }
    }
    int frei = 0;
    for (int i = 0; i < 256; i++) {
        if (!gehoert[i]) {
            frei++;
        }
    }
    GLEICH(bm_free_frames(&a.bm), (uint64_t)frei);
}

TEST(free_mit_umlaufendem_index_wird_abgewiesen)
{
    /* Der Kern des Ganzen: ein verrutschter Index darf NICHT per Umlauf
     * Rahmen am Anfang der Bitmap freigeben. Genau hier wuerde eine
     * ungeprüfte Addition still das Falsche tun. */
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    uint64_t f = bm_alloc_contiguous(&a.bm, 16);
    GLEICH(f, 0);
    GLEICH(bm_free(&a.bm, UINT64_MAX, 1), E_OUT_OF_RANGE);
    GLEICH(bm_free(&a.bm, UINT64_MAX, 2), E_OUT_OF_RANGE);
    GLEICH(bm_free(&a.bm, 1, UINT64_MAX), E_OUT_OF_RANGE);
    GLEICH(bm_free(&a.bm, UINT64_MAX / 2, UINT64_MAX / 2 + 4), E_OUT_OF_RANGE);
    GLEICH(bm_free_frames(&a.bm), 240);
    for (uint64_t i = 0; i < 16; i++) {
        PRUEFE(bm_is_used(&a.bm, i));
    }
    GLEICH(bm_free(&a.bm, 0, 16), E_OK);
    GLEICH(bm_free_frames(&a.bm), 256);
}

TEST(bereiche_hinter_dem_ende_werden_gekappt_nicht_umgelaufen)
{
    Aufbau a;
    aufbau(&a, 100, 2);
    bm_free_range(&a.bm, 0, UINT64_MAX);
    GLEICH(bm_free_frames(&a.bm), 100);
    bm_reserve_range(&a.bm, 90, UINT64_MAX);
    GLEICH(bm_free_frames(&a.bm), 90);
    bm_free_range(&a.bm, UINT64_MAX - 1, UINT64_MAX);
    GLEICH(bm_free_frames(&a.bm), 90);
    bm_reserve_range(&a.bm, UINT64_MAX, UINT64_MAX);
    GLEICH(bm_free_frames(&a.bm), 90);
}

TEST(suchzeiger_zeigt_nach_free_nie_nach_draussen)
{
    Aufbau a;
    aufbau(&a, 256, 4);
    bm_free_range(&a.bm, 0, 256);
    while (bm_alloc(&a.bm) != KEIN_RAHMEN) {
        /* leer laufen */
    }
    GLEICH(bm_free_frames(&a.bm), 0);
    GLEICH(bm_free(&a.bm, 250, 6), E_OK);
    GLEICH(bm_alloc_contiguous(&a.bm, 6), 250);
    GLEICH(bm_free(&a.bm, 0, 256), E_OK);
    GLEICH(bm_free_frames(&a.bm), 256);
    GLEICH(bm_alloc_contiguous(&a.bm, 256), 0);
}

TEST(anforderung_ganz_am_ende_passt_genau)
{
    Aufbau a;
    aufbau(&a, 200, 4); /* Ende mitten im vierten Wort */
    bm_free_range(&a.bm, 0, 200);
    bm_reserve_range(&a.bm, 0, 190);
    GLEICH(bm_alloc_contiguous(&a.bm, 11), KEIN_RAHMEN);
    GLEICH(bm_alloc_contiguous(&a.bm, 10), 190);
    GLEICH(bm_free_frames(&a.bm), 0);
    PRUEFE(bm_is_used(&a.bm, 199));
    PRUEFE(bm_is_used(&a.bm, 200));
}

/* ------------------------------------------------------ Eigenschaftstest */

/* Zeile fuer Zeile derselbe Generator wie libs/osum-mem/src/testrand.rs:
 * LCG mit MMIX-Konstanten, danach die beiden Woerter vertauscht (die unteren
 * Bits eines LCG sind schwach). Damit sehen Rust- und Firn-Fassung die
 * IDENTISCHE Folge -- ein Unterschied im Ergebnis ist dann ein Unterschied im
 * Verwalter, nicht im Zufall. */
static uint64_t lcg_zustand;
static void lcg_neu(uint64_t seed) { lcg_zustand = seed ^ 0x9e3779b97f4a7c15ULL; }
static uint64_t lcg_next(void)
{
    lcg_zustand = lcg_zustand * 6364136223846793005ULL + 1442695040888963407ULL;
    return (lcg_zustand >> 32) | (lcg_zustand << 32);
}
static uint64_t lcg_below(uint64_t n) { return lcg_next() % n; }
static uint64_t lcg_range(uint64_t lo, uint64_t hi) { return lo + lcg_below(hi - lo + 1); }
static bool lcg_chance(uint64_t p) { return lcg_below(100) < p; }

TEST(zufallsfolge_stimmt_mit_referenzmodell_ueberein)
{
    const uint64_t FRAMES = 512;
    for (uint64_t seed = 0; seed < 12; seed++) {
        Aufbau a;
        aufbau(&a, FRAMES, 8);
        bm_free_range(&a.bm, 0, FRAMES);

        bool modell[512];
        memset(modell, 0, sizeof(modell)); /* true = belegt */
        uint64_t lebend_f[512];
        uint64_t lebend_n[512];
        int lebend = 0;
        lcg_neu(seed);

        for (int schritt = 0; schritt < 2000; schritt++) {
            if (lebend == 0 || lcg_chance(55)) {
                uint64_t anzahl = lcg_range(1, 8);
                uint64_t f = bm_alloc_contiguous(&a.bm, anzahl);
                /* Haette das Modell noch Platz gehabt? */
                bool moeglich = false;
                for (uint64_t i = 0; i + anzahl <= FRAMES; i++) {
                    bool alle_frei = true;
                    for (uint64_t k = i; k < i + anzahl; k++) {
                        if (modell[k]) { alle_frei = false; break; }
                    }
                    if (alle_frei) { moeglich = true; break; }
                }
                if (f != KEIN_RAHMEN) {
                    PRUEFE(moeglich);
                    PRUEFE(f + anzahl <= FRAMES);
                    for (uint64_t i = f; i < f + anzahl; i++) {
                        PRUEFE(!modell[i]);
                        modell[i] = true;
                    }
                    lebend_f[lebend] = f;
                    lebend_n[lebend] = anzahl;
                    lebend++;
                } else {
                    PRUEFE(!moeglich);
                }
            } else {
                int idx = (int)lcg_below((uint64_t)lebend);
                uint64_t f = lebend_f[idx], n = lebend_n[idx];
                lebend_f[idx] = lebend_f[lebend - 1];
                lebend_n[idx] = lebend_n[lebend - 1];
                lebend--;
                GLEICH(bm_free(&a.bm, f, n), E_OK);
                for (uint64_t i = f; i < f + n; i++) {
                    modell[i] = false;
                }
            }
            uint64_t belegt = 0;
            for (uint64_t i = 0; i < FRAMES; i++) {
                if (modell[i]) belegt++;
            }
            GLEICH(bm_used_frames(&a.bm), belegt);
            GLEICH(bm_free_frames(&a.bm), FRAMES - belegt);
        }

        for (uint64_t i = 0; i < FRAMES; i++) {
            PRUEFE(bm_is_used(&a.bm, i) == modell[i]);
        }
        for (int i = 0; i < lebend; i++) {
            GLEICH(bm_free(&a.bm, lebend_f[i], lebend_n[i]), E_OK);
        }
        GLEICH(bm_free_frames(&a.bm), FRAMES);
    }
}

TEST(volle_kapazitaet_nach_zufaelligem_verschleiss)
{
    const uint64_t FRAMES = 256;
    Aufbau a;
    aufbau(&a, FRAMES, 4);
    bm_free_range(&a.bm, 0, FRAMES);
    lcg_neu(99);
    uint64_t lebend_f[256], lebend_n[256];
    int lebend = 0;
    for (int i = 0; i < 2000; i++) {
        if (lcg_chance(60)) {
            uint64_t n = lcg_range(1, 16);
            uint64_t f = bm_alloc_contiguous(&a.bm, n);
            if (f != KEIN_RAHMEN) {
                lebend_f[lebend] = f;
                lebend_n[lebend] = n;
                lebend++;
            }
        } else if (lebend > 0) {
            int idx = (int)lcg_below((uint64_t)lebend);
            uint64_t f = lebend_f[idx], n = lebend_n[idx];
            lebend_f[idx] = lebend_f[lebend - 1];
            lebend_n[idx] = lebend_n[lebend - 1];
            lebend--;
            GLEICH(bm_free(&a.bm, f, n), E_OK);
        }
    }
    for (int i = 0; i < lebend; i++) {
        GLEICH(bm_free(&a.bm, lebend_f[i], lebend_n[i]), E_OK);
    }
    GLEICH(bm_free_frames(&a.bm), FRAMES);
    GLEICH(bm_alloc_contiguous(&a.bm, FRAMES), 0);
}

/* --------------------------------------------------------------- Hauptteil */

int main(void)
{
    printf("Pruefstand kernel/firn/bitmap.fi -- dieselben Fragen wie bitmap.rs\n\n");

    lauf_alles_zunaechst_belegt();
    lauf_alloc_und_free_hin_und_zurueck();
    lauf_zusammenhaengend_ueberspringt_belegtes();
    lauf_erschoepfung_meldet_oom();
    lauf_words_needed_rundet_auf();
    lauf_rahmenzahl_muss_kein_vielfaches_von_64_sein();
    lauf_reserve_und_free_sind_idempotent();
    lauf_leere_und_verdrehte_bereiche_tun_nichts();
    lauf_alloc_weist_unmoegliche_anzahlen_ab();
    lauf_free_ausserhalb_wird_abgewiesen();
    lauf_allokation_ueber_wortgrenzen();
    lauf_suchzeiger_laeuft_um_und_nutzt_freigegebenes();
    lauf_zerstueckelung_blockt_grosse_anforderungen();
    lauf_bitmap_mit_einem_einzigen_rahmen();
    lauf_bitmap_ohne_rahmen_vergibt_nie();
    lauf_zu_kleiner_bitspeicher_wird_abgewiesen();
    lauf_allokationen_ueberlappen_nie();
    lauf_free_mit_umlaufendem_index_wird_abgewiesen();
    lauf_bereiche_hinter_dem_ende_werden_gekappt_nicht_umgelaufen();
    lauf_suchzeiger_zeigt_nach_free_nie_nach_draussen();
    lauf_anforderung_ganz_am_ende_passt_genau();
    lauf_zufallsfolge_stimmt_mit_referenzmodell_ueberein();
    lauf_volle_kapazitaet_nach_zufaelligem_verschleiss();

    printf("\n%d bestanden, %d gefallen\n", bestanden, gefallen);
    return gefallen == 0 ? 0 : 1;
}
