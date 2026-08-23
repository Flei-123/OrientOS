# Codegen-Befunde für die Firn-Tempo-Runde

Gefunden beim Portieren des ELF-Prüfteils (23.08.2026, Übersetzer `4536a191`).
Ziel des Nutzers: **Firn soll überall so schnell sein wie Rust.**

Alle Beispiele sind minimal, laufen unverändert durch `firnc` und sind gegen
`cc -O2` gehalten — nicht gegen eine Behauptung.

**Messlage, die dahintersteht** (`tests/firn-elf/tempo.c`, 1 060 000 Aufrufe):

| Fassung | je Aufruf | Verhältnis |
|---|---|---|
| Rust-Verfahren (`checked_*`, `cc -O2`) | 79 ns | 1,00× |
| **Firn `dev-fast`** (was osum baut) | **651 ns** | **8,25×** |
| Firn `release-fast` (Prüfungen aus) | 136 ns | 1,97× |

Die Aufteilung sagt, wo es sitzt: **rund drei Viertel des Aufschlags kommen aus
T-01**, der Rest aus T-02 und T-03.

---

## T-01 · Die geprüfte Addition legt beide Operanden auf den Stapel — auf dem **Erfolgspfad**

**Das ist der teuerste Befund.** Ein Parser rechnet in jeder Zeile; hier kostet
jede Rechnung drei zusätzliche Speicherzugriffe, auch wenn nichts überläuft.

```firn
profile kernel
#[export_c]
fn summe(a: u64, b: u64) -> u64 {
    return a + b
}
```

`firnc --opt-level=dev-fast`:

```
  8:  mov    %rdi,%r8
  b:  mov    %rsi,%r9
  e:  mov    %r8,%rax
 11:  mov    %r9,%rcx
 14:  push   %rax          <-- beide Operanden auf den Stapel,
 15:  push   %rcx              nur damit der Panikpfad sie hätte
 16:  add    %rcx,%rax
 19:  jb     21
 1b:  add    $0x10,%rsp    <-- und im Normalfall wieder herunter
 1f:  jmp    3f
```

**Was gcc/rustc daraus machen:** die Operanden bleiben in ihren Registern, der
Prüfsprung geht in einen Block *außerhalb* des heißen Pfads, und dieser Block
liest sie von dort. Der Erfolgspfad kostet **`add` + `jo`** und sonst nichts.

**Vorschlag:** Die Rettung der Operanden gehört in den **kalten** Block hinter
dem Sprung, nicht davor. Sind die Register dort schon überschrieben, reicht es,
sie genau in diesem Zweig zu rekonstruieren oder zu spillen — der Zweig läuft
nur, wenn das Programm ohnehin abbricht.

**Erwarteter Gewinn:** 3 Speicherzugriffe je Rechnung. Beim ELF-Parser ist das
der Unterschied zwischen 651 ns und etwas nahe 136 ns.

---

## T-02 · Jede Funktion baut einen Stapelrahmen auf, auch ohne lokale Variablen

```firn
profile kernel
fn rd8(p: u64, off: u64) -> u64 {
    let b: u8 = *((p + off) as *mut u8)
    return b as u64
}
#[export_c]
fn rd16(p: u64, off: u64) -> u64 {
    return rd8(p, off) | (rd8(p, off + 1) << 8)
}
```

`firnc --opt-level=release-fast` — **19 Instruktionen**:

```
 22:  push   %rbp
 23:  mov    %rsp,%rbp
 26:  sub    $0xc0,%rsp     <-- 192 Oktette für eine Funktion ohne Locals
 ...
 5f:  mov    %rbp,%rsp
 62:  pop    %rbp
 63:  ret
```

`cc -O2`, dasselbe Verfahren — **5 Instruktionen**:

```
 10:  movzbl 0x1(%rdi,%rsi,1),%eax
 15:  movzbl (%rdi,%rsi,1),%edx
 19:  shl    $0x8,%rax
 1d:  or     %rdx,%rax
 20:  ret
```

Zwei Dinge stecken darin:

* **Der Rahmen wird immer aufgebaut**, auch wenn die Funktion nichts auf dem
  Stapel ablegt. Rust und C lassen ihn weg, sobald er unnötig ist.
* **`sub $0xc0,%rsp` ist zu groß.** 192 Oktette für eine Funktion mit zwei
  Parametern und einer Zwischenrechnung deutet darauf hin, dass die
  Rahmengröße aus einer Obergrenze kommt und nicht aus dem echten Bedarf.

**Anmerkung:** osum baut mit `-C force-frame-pointers=yes`, weil der Backtrace
über `rbp` läuft. Das rechtfertigt `push rbp`/`mov rsp,rbp` — **nicht** das
`sub $0xc0,%rsp` und nicht die Größe.

---

## T-03 · Werte wandern durch drei bis vier Register, bevor sie benutzt werden

Aus demselben `rd16`, `release-fast`:

```
 33:  movzbl (%r9,%r8,1),%r11d
 38:  movzbl %r11b,%eax        <-- r11 -> rax
 3c:  mov    %rax,%r10         <-- rax -> r10
 ...
 4c:  mov    %rax,%r8          <-- rax -> r8
 4f:  mov    %r8,%r11          <-- r8  -> r11
 52:  shl    $0x8,%r11
 56:  mov    %r10,%r8          <-- r10 -> r8
 59:  or     %r11,%r8
 5c:  mov    %r8,%rax          <-- r8  -> rax
```

**Acht `mov` zwischen Registern** für eine Rechnung, die aus zwei Ladevorgängen,
einem `shl` und einem `or` besteht. Jede Zuweisung bekommt ein frisches
Register, statt das vorhandene weiterzuverwenden.

**Vorschlag:** Ein Durchgang für **Copy-Propagation und Register-Coalescing** —
`mov ra, rb` fällt weg, wenn `ra` danach nicht mehr gebraucht wird. Das ist
üblicherweise der billigste große Gewinn nach der Registerallokation.

Das `movzbl %r11b,%eax` direkt nach einem `movzbl`, das schon nullerweitert
hat, gehört in denselben Topf: die Nullerweiterung ist bereits geschehen.

---

## Was **nicht** das Problem ist

* **Die Prüfungen selbst.** Ein `add`+`jo` kostet auf modernen Kernen praktisch
  nichts, weil der Sprung nie genommen wird. Teuer ist nur, wie sie hier
  *drumherum* gebaut sind (T-01).
* **Die Sprache.** Nichts an diesen drei Punkten hängt an Firns Semantik — es
  sind Durchgänge, die dem Backend fehlen.

## Reihenfolge, wenn nur eines gemacht wird

**T-01 zuerst.** Es ist der größte Anteil, betrifft jeden gerechneten Ausdruck
und ändert nichts an der Semantik — nur, *wo* die Operanden für den Panikpfad
herkommen.
