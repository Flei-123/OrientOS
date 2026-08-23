/* tests/firn-elf/tempo.c -- misst den ELF-Pruefteil: Firn gegen das
 * Rust-Verfahren, unter identischer Last.
 *
 * Die Rust-Fassung wird hier ein zweites Mal geschrieben -- Zeile fuer Zeile
 * dasselbe Verfahren, uebersetzt vom selben C-Compiler mit denselben
 * Schaltern. So misst der Vergleich das VERFAHREN und den vom jeweiligen
 * Uebersetzer erzeugten Code, nicht den Unterschied zweier Sprachlaufzeiten.
 *
 * Die Last: jede der 53 Falldateien wird N-mal geparst. Das ist die echte
 * Arbeit des Pruefteils -- Kopf lesen, Programmkoepfe durchgehen, gegen alles
 * schon Eingetragene pruefen. Auch die abgewiesenen Faelle zaehlen mit: ein
 * Parser, der Muell schnell abweist, ist genau das, was ein Kernel braucht.
 */

#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <time.h>
#include <dirent.h>

#define MAX_SEGMENTS 16
#define EHDR_LEN 64
#define PHDR_LEN 56
#define PT_LOAD 1
#define PT_INTERP 3
#define ET_EXEC 2
#define EM_EXPECTED 62
#define PAGE 4096ULL
#define USER_BASE 0x400000ULL
#define USER_LIMIT 0x800000000000ULL

typedef struct {
    uint64_t vaddr, file_len, mem_len, file_off, flags;
} ElfSegment;

typedef struct {
    uint64_t entry, count;
    ElfSegment segments[MAX_SEGMENTS];
} ElfImage;

/* --- die Firn-Seite --- */
extern uint64_t elf_parse(uint64_t p, uint64_t len, ElfImage *img);

void osum_panic(const char *msg, uint32_t len, uint64_t a, uint64_t b, uint64_t art)
{
    (void)a; (void)b; (void)art;
    printf("PANIK in der Messung: %.*s\n", (int)len, msg);
    exit(1);
}

/* --- die Rust-Seite, hier nachgebildet ---
 * Genau das Verfahren aus dem alten kernel/src/kcore/elf.rs, einschliesslich
 * der checked_*-Pruefungen (die dort von Hand standen). */

static inline uint16_t rd16(const uint8_t *b, size_t o) { return (uint16_t)(b[o] | (b[o+1] << 8)); }
static inline uint32_t rd32(const uint8_t *b, size_t o) {
    return (uint32_t)b[o] | ((uint32_t)b[o+1] << 8) | ((uint32_t)b[o+2] << 16) | ((uint32_t)b[o+3] << 24);
}
static inline uint64_t rd64(const uint8_t *b, size_t o) {
    uint64_t v = 0;
    for (int i = 7; i >= 0; i--) { v = (v << 8) | b[o + i]; }
    return v;
}
static inline bool ist_zp(uint64_t v) { return v != 0 && (v & (v - 1)) == 0; }

static uint64_t r_parse(const uint8_t *bytes, size_t len, ElfImage *img)
{
    if (len < EHDR_LEN) return 1;
    if (bytes[0] != 0x7f || bytes[1] != 'E' || bytes[2] != 'L' || bytes[3] != 'F') return 2;
    if (bytes[4] != 2) return 3;
    if (bytes[5] != 1) return 4;
    if (rd16(bytes, 16) != ET_EXEC) return 5;
    if (rd16(bytes, 18) != EM_EXPECTED) return 6;

    uint64_t entry = rd64(bytes, 24);
    uint64_t phoff = rd64(bytes, 32);
    size_t phentsize = rd16(bytes, 54);
    size_t phnum = rd16(bytes, 56);
    if (phentsize != PHDR_LEN || phnum == 0) return 7;
    uint64_t tab = (uint64_t)phnum * PHDR_LEN;
    if (phoff > UINT64_MAX - tab) return 7;
    if (phoff + tab > len) return 7;

    img->entry = entry;
    img->count = 0;
    for (size_t i = 0; i < phnum; i++) {
        size_t p = (size_t)phoff + i * PHDR_LEN;
        uint32_t ptype = rd32(bytes, p);
        if (ptype == PT_INTERP) return 14;
        if (ptype != PT_LOAD) continue;
        if (img->count == MAX_SEGMENTS) return 16;

        uint32_t flags = rd32(bytes, p + 4);
        uint64_t off = rd64(bytes, p + 8);
        uint64_t vaddr = rd64(bytes, p + 16);
        uint64_t filesz = rd64(bytes, p + 32);
        uint64_t memsz = rd64(bytes, p + 40);
        uint64_t align = rd64(bytes, p + 48);

        if (filesz > memsz || memsz == 0) return 12;
        if (align > 1) {
            if (!ist_zp(align) || vaddr % align != off % align) return 13;
        }
        if (off > UINT64_MAX - filesz) return 8;
        if (off + filesz > len) return 8;
        if (vaddr > UINT64_MAX - memsz) return 9;
        uint64_t vend = vaddr + memsz;
        if (!(vaddr >= USER_BASE && vaddr < USER_LIMIT) || vend > USER_LIMIT) return 9;

        uint64_t lo = vaddr & ~(PAGE - 1);
        uint64_t hi = (vend + PAGE - 1) & ~(PAGE - 1);
        for (uint64_t k = 0; k < img->count; k++) {
            uint64_t pv = img->segments[k].vaddr;
            uint64_t pend = pv + img->segments[k].mem_len;
            if (vaddr < pend && pv < vend) return 10;
            uint64_t plo = pv & ~(PAGE - 1);
            uint64_t phi = (pend + PAGE - 1) & ~(PAGE - 1);
            if (lo < phi && plo < hi) return 11;
        }
        img->segments[img->count].vaddr = vaddr;
        img->segments[img->count].file_len = filesz;
        img->segments[img->count].mem_len = memsz;
        img->segments[img->count].file_off = off;
        img->segments[img->count].flags = flags;
        img->count++;
    }
    if (img->count == 0) return 7;
    bool ok = false;
    for (uint64_t j = 0; j < img->count; j++) {
        uint64_t sv = img->segments[j].vaddr;
        if (entry >= sv && entry < sv + img->segments[j].mem_len
            && (img->segments[j].flags & 1)) { ok = true; }
    }
    if (!ok) return 15;
    return 0;
}

/* --------------------------------------------------------- die Arbeitslast */

#define MAX_FAELLE 128
#define MAX_LEN 8192
static uint8_t inhalt[MAX_FAELLE][MAX_LEN];
static size_t laenge[MAX_FAELLE];
static int anzahl_faelle = 0;

static double jetzt(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec / 1e9;
}

#define RUNDEN 5
#define WIEDERHOLUNGEN 20000

int main(int argc, char **argv)
{
    const char *dir = argc > 1 ? argv[1] : "tests/firn-elf/faelle";
    char pfad[1024];
    snprintf(pfad, sizeof(pfad), "%s/erwartet.txt", dir);
    FILE *liste = fopen(pfad, "r");
    if (!liste) { fprintf(stderr, "%s fehlt\n", pfad); return 1; }

    char zeile[512], name[256];
    unsigned erwartet;
    while (fgets(zeile, sizeof(zeile), liste) && anzahl_faelle < MAX_FAELLE) {
        if (sscanf(zeile, "%255s %u", name, &erwartet) < 2) continue;
        snprintf(pfad, sizeof(pfad), "%s/%s", dir, name);
        FILE *f = fopen(pfad, "rb");
        if (!f) continue;
        laenge[anzahl_faelle] = fread(inhalt[anzahl_faelle], 1, MAX_LEN, f);
        fclose(f);
        anzahl_faelle++;
    }
    fclose(liste);

    double firn_best = 1e9, rust_best = 1e9;
    volatile uint64_t senke = 0;

    for (int r = 0; r < RUNDEN; r++) {
        ElfImage img;
        double t0 = jetzt();
        for (int w = 0; w < WIEDERHOLUNGEN; w++) {
            for (int i = 0; i < anzahl_faelle; i++) {
                senke += elf_parse((uint64_t)(uintptr_t)inhalt[i], laenge[i], &img);
            }
        }
        double f = jetzt() - t0;

        t0 = jetzt();
        for (int w = 0; w < WIEDERHOLUNGEN; w++) {
            for (int i = 0; i < anzahl_faelle; i++) {
                senke += r_parse(inhalt[i], laenge[i], &img);
            }
        }
        double u = jetzt() - t0;

        if (f < firn_best) firn_best = f;
        if (u < rust_best) rust_best = u;
    }

    long gesamt = (long)WIEDERHOLUNGEN * anzahl_faelle;
    printf("Tempovergleich ELF-Pruefteil\n");
    printf("  Last     : %d Faelle x %d Durchlaeufe = %ld Aufrufe, bestes von %d\n\n",
           anzahl_faelle, WIEDERHOLUNGEN, gesamt, RUNDEN);
    printf("  Firn (geprüfte Arithmetik) : %.4f s  (%.0f ns je Aufruf)\n",
           firn_best, firn_best / gesamt * 1e9);
    printf("  Rust-Verfahren (checked_*) : %.4f s  (%.0f ns je Aufruf)\n",
           rust_best, rust_best / gesamt * 1e9);
    printf("\n  Verhaeltnis: Firn braucht %.2fx der Zeit\n", firn_best / rust_best);
    if (senke == 0xdeadbeef) printf("!");
    return 0;
}
