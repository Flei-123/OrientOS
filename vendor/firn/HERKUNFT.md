# Woher Firn kommt -- und warum der Hash hier nicht ins Netz zeigt

**Firn steht seit dem 05.09.2026 unter der Mozilla Public License 2.0.**
Das oeffentliche Repository ist:

    https://github.com/Flei-123/Firn

Es beginnt mit einem einzigen Commit `bd448a306` ("Firn 0.2 -- neuer
Anfang unter MPL-2.0"). Bis zu diesem Tag stand Firn unter MIT; diese
Vorgeschichte liegt jetzt im **privaten** Archiv `Flei-123/FirnOld` und ist
oeffentlich nicht mehr abrufbar.

## Was das fuer die Datei COMMIT bedeutet

`vendor/firn/COMMIT` nagelt OrientOS auf den Firn-Stand

    4536a191d09c755f67652c7ca13d22f765d8fa51

fest. Dieser Hash stammt aus der **alten** Historie. Er existiert weiterhin
in der Arbeitskopie des Autors und im Archiv FirnOld, aber **nicht** im neuen
oeffentlichen Repository. Wer von aussen baut, kann diesen genauen Stand
daher nicht holen.

Das ist bewusst so gelassen und kein Versehen:

* Der Pin zeigt auf einen Uebersetzer, gegen den dieses Repository
  **nachweislich gruen getestet** ist. Ihn auf gut Glueck auf den neuen
  oeffentlichen Anfangsstand umzubiegen wuerde diesen Nachweis wegwerfen --
  der neue oeffentliche Stand ist an einigen Stellen **aelter** als der hier
  angenagelte.
* Sobald die laufende Zusammenfuehrung in Firn abgeschlossen und
  veroeffentlicht ist, wird `COMMIT` auf einen Hash aus dem **neuen**
  oeffentlichen Repository gezogen. Erst dann ist der Bau von aussen
  vollstaendig reproduzierbar.

## Lizenzlage

Firn ist MPL-2.0. Das ist mit der GPL vertraeglich: Firn darf in diesem
GPL-lizenzierten Projekt benutzt und mitverteilt werden. Geaenderte
Firn-Dateien muessen offengelegt bleiben, dieses Projekt bleibt davon
unberuehrt.
