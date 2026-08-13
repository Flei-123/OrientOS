# tests/step-17-ring3.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis der unprivilegierten Ebene: ein Programm laeuft wirklich in Ring 3
# (Selektor mit RPL 3 und CPL 3, an einem echten Ausnahmerahmen gemessen),
# gibt ueber ein explizit uebergebenes Handle etwas aus, beendet sich per
# Systemaufruf — und ein Zugriff auf eine Kerneladresse wird sauber
# abgewiesen, ohne den Kernel anzuhalten.
step "Ring 3: unprivilegiertes Programm, Systemaufruf, Negativtest"
run ./run-qemu.sh --test-ring3
