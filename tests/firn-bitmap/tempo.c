/* tests/firn-bitmap/tempo.c -- misst kernel/firn/bitmap.fi gegen
 * libs/osum-mem/src/bitmap.rs unter IDENTISCHER Last.
 *
 * Die Rust-Fassung wird nicht nachgebaut, sondern hier ein zweites Mal
 * geschrieben -- Zeile fuer Zeile dasselbe Verfahren, uebersetzt vom selben
 * C-Compiler mit denselben Schaltern. Damit misst der Vergleich das
 * VERFAHREN und den vom jeweiligen Uebersetzer erzeugten Code, nicht den
 * Unterschied zwischen zwei Sprachlaufzeiten.
 *
 * Warum das die faire Messung ist: Firns Objekt ist freistehend und ohne
 * Optimierungsstufe uebersetzt; die Rust-Fassung laeuft im Kernel mit
 * `-O`. Ein Vergleich Firn-Objekt gegen rustc-Objekt waere ein Vergleich
 * zweier Optimierer. Diese Messung sagt: wie viel kostet der Weg ueber die
 * Firn-Fassung, so wie sie heute wirklich im Kernel liegt.
 *
 * Ausgegeben wird die Zeit fuer eine feste Folge von Anforderungen und
 * Rueckgaben, gemittelt ueber mehrere Durchlaeufe.
 */

#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <time.h>

typedef struct {
    uint64_t bits;
    uint64_t frames;
    uint64_t used;
    uint64_t cursor;
} Bitmap;

#define KEIN_RAHMEN 0xFFFFFFFFFFFFFFFFULL

/* --- die Firn-Seite --- */
extern bool     bm_init(Bitmap *bm, uint64_t bits, uint64_t woerter, uint64_t frames);
extern void     bm_free_range(Bitmap *bm, uint64_t start, uint64_t ende);
extern uint64_t bm_alloc_contiguous(Bitmap *bm, uint64_t anzahl);
extern uint64_t bm_free(Bitmap *bm, uint64_t index, uint64_t anzahl);
extern uint64_t bm_free_frames(Bitmap *bm);

void osum_panic(const char *msg, uint32_t len, uint64_t a, uint64_t b, uint64_t art)
{
    (void)a; (void)b; (void)art;
    printf("PANIK in der Messung: %.*s\n", (int)len, msg);
    exit(1);
}

/* --- die Rust-Seite, hier nachgebildet ---
 * Genau das Verfahren aus libs/osum-mem/src/bitmap.rs, einschliesslich des
 * wortweisen Schnellvorlaufs und der Budget-Grenze. Ohne Ueberlaufpruefung,
 * so wie rustc es im Release-Profil erzeugt. */

typedef struct {
    uint64_t *bits;
    size_t frames;
    size_t used;
    size_t cursor;
} RBitmap;

static void r_init(RBitmap *b, uint64_t *speicher, size_t woerter, size_t frames)
{
    for (size_t i = 0; i < woerter; i++) {
        speicher[i] = UINT64_MAX;
    }
    b->bits = speicher;
    b->frames = frames;
    b->used = frames;
    b->cursor = 0;
}

static inline bool r_test(const RBitmap *b, size_t i)
{
    return (b->bits[i / 64] >> (i % 64)) & 1;
}
static inline void r_set(RBitmap *b, size_t i) { b->bits[i / 64] |= 1ULL << (i % 64); }
static inline void r_clear(RBitmap *b, size_t i) { b->bits[i / 64] &= ~(1ULL << (i % 64)); }

static void r_free_range(RBitmap *b, size_t start, size_t ende)
{
    size_t stop = ende < b->frames ? ende : b->frames;
    for (size_t i = start; i < stop; i++) {
        if (r_test(b, i)) { r_clear(b, i); b->used--; }
    }
}

static size_t r_alloc_contiguous(RBitmap *b, size_t anzahl)
{
    if (anzahl == 0 || anzahl > b->frames) {
        return SIZE_MAX;
    }
    size_t letzter_start = b->frames - anzahl;
    size_t i = b->cursor > letzter_start ? 0 : b->cursor;
    size_t budget = letzter_start + 1;
    while (budget > 0) {
        if (i > letzter_start) { i = 0; continue; }
        if (anzahl == 1 && i % 64 == 0 && b->bits[i / 64] == UINT64_MAX) {
            i += 64;
            budget = budget > 64 ? budget - 64 : 0;
            continue;
        }
        bool frei = true;
        size_t j = i;
        while (j < i + anzahl) {
            if (r_test(b, j)) { frei = false; break; }
            j++;
        }
        if (frei) {
            for (size_t k = i; k < i + anzahl; k++) { r_set(b, k); }
            b->used += anzahl;
            b->cursor = i + anzahl;
            return i;
        }
        size_t verbraucht = j + 1 - i;
        budget = budget > verbraucht ? budget - verbraucht : 0;
        i = j + 1;
    }
    return SIZE_MAX;
}

static int r_free(RBitmap *b, size_t index, size_t anzahl)
{
    if (anzahl > SIZE_MAX - index) { return 2; }
    if (index + anzahl > b->frames) { return 2; }
    for (size_t i = index; i < index + anzahl; i++) {
        if (r_test(b, i)) { r_clear(b, i); b->used--; }
    }
    if (index < b->cursor) { b->cursor = index; }
    return 0;
}

/* --------------------------------------------------------- die Arbeitslast */

/* Derselbe LCG wie libs/osum-mem/src/testrand.rs. */
static uint64_t lcg;
static void lcg_neu(uint64_t seed) { lcg = seed ^ 0x9e3779b97f4a7c15ULL; }
static uint64_t lcg_next(void)
{
    lcg = lcg * 6364136223846793005ULL + 1442695040888963407ULL;
    return (lcg >> 32) | (lcg << 32);
}

#define FRAMES 65536
#define WOERTER (FRAMES / 64)
#define SCHRITTE 40000

static double jetzt(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec / 1e9;
}

/* Die Folge der Anforderungen wird EINMAL erzeugt und beiden Fassungen
 * unveraendert vorgelegt -- sonst misst man den Zufallsgenerator mit. */
static uint64_t folge_anzahl[SCHRITTE];
static uint8_t  folge_ist_alloc[SCHRITTE];
static uint64_t folge_index[SCHRITTE];

static void folge_bauen(void)
{
    lcg_neu(12345);
    for (int i = 0; i < SCHRITTE; i++) {
        folge_ist_alloc[i] = (lcg_next() % 100) < 55;
        folge_anzahl[i] = 1 + lcg_next() % 8;
        folge_index[i] = lcg_next();
    }
}

static double firn_lauf(uint64_t *speicher, uint64_t *lebend_f, uint64_t *lebend_n)
{
    Bitmap bm;
    bm_init(&bm, (uint64_t)(uintptr_t)speicher, WOERTER, FRAMES);
    bm_free_range(&bm, 0, FRAMES);
    int lebend = 0;
    double t0 = jetzt();
    for (int i = 0; i < SCHRITTE; i++) {
        if (lebend == 0 || folge_ist_alloc[i]) {
            uint64_t f = bm_alloc_contiguous(&bm, folge_anzahl[i]);
            if (f != KEIN_RAHMEN) {
                lebend_f[lebend] = f;
                lebend_n[lebend] = folge_anzahl[i];
                lebend++;
            }
        } else {
            int idx = (int)(folge_index[i] % (uint64_t)lebend);
            bm_free(&bm, lebend_f[idx], lebend_n[idx]);
            lebend_f[idx] = lebend_f[lebend - 1];
            lebend_n[idx] = lebend_n[lebend - 1];
            lebend--;
        }
    }
    double t1 = jetzt();
    /* Ergebnis anfassen, damit nichts wegoptimiert wird. */
    if (bm_free_frames(&bm) == 0xdeadbeef) { printf("!"); }
    return t1 - t0;
}

static double rust_lauf(uint64_t *speicher, uint64_t *lebend_f, uint64_t *lebend_n)
{
    RBitmap bm;
    r_init(&bm, speicher, WOERTER, FRAMES);
    r_free_range(&bm, 0, FRAMES);
    int lebend = 0;
    double t0 = jetzt();
    for (int i = 0; i < SCHRITTE; i++) {
        if (lebend == 0 || folge_ist_alloc[i]) {
            size_t f = r_alloc_contiguous(&bm, (size_t)folge_anzahl[i]);
            if (f != SIZE_MAX) {
                lebend_f[lebend] = f;
                lebend_n[lebend] = folge_anzahl[i];
                lebend++;
            }
        } else {
            int idx = (int)(folge_index[i] % (uint64_t)lebend);
            r_free(&bm, lebend_f[idx], lebend_n[idx]);
            lebend_f[idx] = lebend_f[lebend - 1];
            lebend_n[idx] = lebend_n[lebend - 1];
            lebend--;
        }
    }
    double t1 = jetzt();
    if (bm.used == 0xdeadbeef) { printf("!"); }
    return t1 - t0;
}

int main(void)
{
    static uint64_t speicher[WOERTER];
    static uint64_t lebend_f[SCHRITTE], lebend_n[SCHRITTE];

    folge_bauen();

    const int RUNDEN = 7;
    double firn_best = 1e9, rust_best = 1e9;
    double firn_summe = 0, rust_summe = 0;

    /* Abwechselnd, damit Taktverhalten und Cache beide gleich treffen. */
    for (int r = 0; r < RUNDEN; r++) {
        double f = firn_lauf(speicher, lebend_f, lebend_n);
        double u = rust_lauf(speicher, lebend_f, lebend_n);
        if (f < firn_best) firn_best = f;
        if (u < rust_best) rust_best = u;
        firn_summe += f;
        rust_summe += u;
    }

    printf("Tempovergleich Bitmap-Rahmenverwalter\n");
    printf("  Last     : %d Schritte, %d Rahmen, dieselbe Folge fuer beide\n",
           SCHRITTE, FRAMES);
    printf("  Runden   : %d\n\n", RUNDEN);
    printf("  Firn (geprüfte Arithmetik) : bestes %.4f s   Mittel %.4f s\n",
           firn_best, firn_summe / RUNDEN);
    printf("  Rust-Verfahren (ungeprüft) : bestes %.4f s   Mittel %.4f s\n",
           rust_best, rust_summe / RUNDEN);
    printf("\n  Verhaeltnis (bestes): Firn braucht %.2fx der Zeit\n",
           firn_best / rust_best);
    return 0;
}
