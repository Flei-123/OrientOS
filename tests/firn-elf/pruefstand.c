/* tests/firn-elf/pruefstand.c -- faehrt DIESELBEN 53 Faelle aus faelle/ gegen
 * die FIRN-Fassung des ELF-Pruefteils.
 *
 * Der Massstab ist derselbe wie fuer die Rust-Fassung: dieselben Dateien,
 * dieselbe erwartet.txt. Ein Unterschied im Ergebnis ist damit ein
 * Unterschied im Parser -- nicht im Testaufbau.
 *
 * Warum C: das Firn-Objekt wird im Kernelprofil uebersetzt, freistehend, ohne
 * libc. Hier wird GENAU DIESES Objekt gelinkt, nicht eine host-taugliche
 * Zweitfassung.
 *
 * Uebersetzen und laufen lassen: tests/firn-elf/lauf.sh
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

#define MAX_SEGMENTS 16

/* Feld fuer Feld wie `struct ElfSegment` / `struct ElfImage` in elf.fi. */
typedef struct {
    uint64_t vaddr;
    uint64_t file_len;
    uint64_t mem_len;
    uint64_t file_off;
    uint64_t flags;
} ElfSegment;

typedef struct {
    uint64_t entry;
    uint64_t count;
    ElfSegment segments[MAX_SEGMENTS];
} ElfImage;

extern uint64_t elf_parse(uint64_t p, uint64_t len, ElfImage *img);
extern uint64_t elf_max_segments(void);

/* Firns geprüfte Arithmetik landet hier. Im Pruefstand ist JEDER Anschlag ein
 * fehlgeschlagener Test: der Parser bekommt absichtlich Zahlen dicht an
 * UINT64_MAX vorgesetzt und muss sie mit einem FEHLERWERT abweisen, nicht mit
 * einem Abbruch. Genau das ist der Unterschied zwischen "Muell fuehrt zu einem
 * Fehlerwert" und "Muell haelt den Kernel an". */
static const char *laufender_fall = "";

void osum_panic(const char *msg, uint32_t len, uint64_t a, uint64_t b, uint64_t art)
{
    printf("\n  [FEHL] %s: Firns Pruefung schlug an -- der Parser haette den\n"
           "         Fall mit einem Fehlerwert abweisen muessen, nicht abbrechen.\n",
           laufender_fall);
    printf("         %.*s\n", (int)len, msg);
    printf("         a=%llu b=%llu art=%llu\n",
           (unsigned long long)a, (unsigned long long)b, (unsigned long long)art);
    exit(1);
}

static unsigned char puffer[1 << 20];

int main(int argc, char **argv)
{
    const char *dir = argc > 1 ? argv[1] : "tests/firn-elf/faelle";
    char pfad[1024];

    snprintf(pfad, sizeof(pfad), "%s/erwartet.txt", dir);
    FILE *liste = fopen(pfad, "r");
    if (!liste) {
        fprintf(stderr, "%s fehlt -- erst tests/firn-elf/faelle.py laufen lassen\n", pfad);
        return 1;
    }

    printf("Massstab gegen die FIRN-Fassung (kernel/firn/elf.fi)\n\n");

    if (elf_max_segments() != MAX_SEGMENTS) {
        printf("  [FEHL] elf.fi kennt %llu Segmente, der Pruefstand %d\n",
               (unsigned long long)elf_max_segments(), MAX_SEGMENTS);
        return 1;
    }

    int bestanden = 0, gefallen = 0;
    char zeile[512];
    while (fgets(zeile, sizeof(zeile), liste)) {
        char name[256], beschreibung[256];
        unsigned erwartet;
        if (sscanf(zeile, "%255s %u %255[^\n]", name, &erwartet, beschreibung) < 2) {
            continue;
        }
        laufender_fall = name;

        snprintf(pfad, sizeof(pfad), "%s/%s", dir, name);
        FILE *f = fopen(pfad, "rb");
        if (!f) {
            printf("  [FEHL] %-34s Falldatei fehlt\n", name);
            gefallen++;
            continue;
        }
        size_t n = fread(puffer, 1, sizeof(puffer), f);
        fclose(f);

        ElfImage img;
        memset(&img, 0, sizeof(img));
        /* Eine Datei der Laenge 0 hat keinen gueltigen Zeiger -- der Parser
         * darf ihn dann auch nicht anfassen. Genau das wird hier geprueft,
         * indem trotzdem ein Zeiger uebergeben wird. */
        uint64_t got = elf_parse((uint64_t)(uintptr_t)puffer, (uint64_t)n, &img);

        if (got == erwartet) {
            bestanden++;
            printf("  [ ok ] %-34s %s\n", name, beschreibung);
        } else {
            gefallen++;
            printf("  [FEHL] %-34s lieferte %llu, erwartet %u  (%s)\n",
                   name, (unsigned long long)got, erwartet, beschreibung);
        }
    }
    fclose(liste);

    printf("\n%d bestanden, %d gefallen\n", bestanden, gefallen);
    return gefallen == 0 ? 0 : 1;
}
