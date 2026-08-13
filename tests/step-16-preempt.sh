# tests/step-16-preempt.sh — wird von test.sh gesourct, nicht direkt gestartet.
# Nachweis der Verdraengung: der Zeitgeber nimmt Zaehlschleifen die CPU weg,
# ohne dass diese je `yield` rufen; Prioritaeten verteilen die Ticks messbar.
step "Verdraengung: Wechsel ohne freiwilliges yield, Prioritaeten wirken"
run ./run-qemu.sh --test-preempt
