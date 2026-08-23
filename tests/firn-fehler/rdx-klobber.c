#include <stdio.h>
#include <stdint.h>
typedef struct { uint64_t a, b, c; } S;
extern void init(S*, uint64_t, uint64_t, uint64_t);
void osum_panic(const char*m,uint32_t l,uint64_t x,uint64_t y,uint64_t k){(void)m;(void)l;(void)x;(void)y;(void)k;}
int main(void){
    static uint64_t worte[4];
    S s = {0,0,0};
    init(&s, (uint64_t)(uintptr_t)worte, 4, 256);
    int ok = (s.b==256 && s.c==256 && s.a==(uint64_t)(uintptr_t)worte);
    printf("init(bits,4,256) -> a=%s b=%llu c=%llu  (erwartet b=c=256)%s\n",
        s.a==(uint64_t)(uintptr_t)worte?"richtig":"FALSCH",
        (unsigned long long)s.b,(unsigned long long)s.c, ok?"":"   << FALSCH");
    return ok?0:1;
}
