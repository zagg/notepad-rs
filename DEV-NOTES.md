# Notes de développement

Mise à jour : 27/10/2026 — **toutes les batteries au vert** (func 4/4, func3 12/12,
tbtest 3/3, verify 10/10). Voir README.md pour la vue d'ensemble et le ledger de preuves REA.

## Bugs de cause racine résolus (les 3 grandes familles)

1. **« Fichier vide »** (fenêtre ouverte mais contenu invisible) : `app().hwnd` n'était
   pas encore enregistré pendant `WM_CREATE`/premier `WM_SIZE` (dispatchés *dans*
   `CreateWindowExW`). Corrigé en assignant `app().hwnd = hwnd` en tout début de `WM_CREATE`.

2. **Rechercher / Atteindre la ligne inopérants** : constantes `EM_*` décalées dans
   `richedit.rs` (ex. `EM_GETSEL=0x00B1` qui est en fait `EM_SETSEL`, `EM_SETSEL=0x00C1`
   qui est `EM_LINELENGTH` → chaque mise à jour de barre d'état réinitialisait la
   sélection, et find/goto ne sélectionnaient jamais rien). Corrigé aux valeurs
   winuser.h (base 0x00B0) + passage aux messages natifs RichEdit `EM_EXGETSEL` /
   `EM_EXSETSEL` (WM_USER+52/+55, `CHARRANGE`) dans status.rs et find.rs.

3. **Autosave ne restaurait rien** : le masque d'événements était réglé avec des
   constantes `ENM_*` inventées (`ENM_CHANGE=0x00300000` au lieu de `0x1`,
   `EN_SELCHANGE=0x0101` au lieu de `0x0702`) → RichEdit n'envoyait jamais
   `EN_CHANGE`, le drapeau `modified` restait à false, `save_all()` ne sautait
   aucun document… par le test « non modifié ». Valeurs corrigées d'après les
   métadonnées Win32 officielles (`windows-0.58` : `EM_SETEVENTMASK=1093`,
   `ENM_CHANGE=1`, `ENM_SELCHANGE=0x80000`, `EN_SELCHANGE=0x0702`, `EN_CHANGE=0x0300`).
   Le traçage cross-process qui avait trompé le diagnostic : `EM_GETMODIFY` renvoyait
   bien `-1` côté contrôle, mais l'état applicatif ne le savait pas.

## Barre d'outils de mise en forme — SUR la ligne des menus (refonte 27/10)

- **Plus de fenêtre enfant de 40 px** : les boutons sont des éléments
  `MF_OWNERDRAW` ajoutés **après « Aide » dans la vraie barre de menus**
  (`AppendMenuW` dans `build_menu`). Zéro ligne perdue, comme l'original.
- Rendu `WM_MEASUREITEM`/`WM_DRAWITEM` (CtlType `ODT_MENU`) : fond `COLOR_MENU`,
  surbrillance `COLOR_MENUHILIGHT` à la sélection, glyphe centré + chevron,
  séparateur vertical au milieu de l'élément « espaceur » (id 5000).
- Icônes **Segoe MDL2 Assets** : List E8FD, Link E71B, Grid E80A, Font E8D2,
  Edit E70F, Share E72D, Contact E77B, Setting E713, ChevronDown E70D ;
  B/I/S en Segoe UI gras/italique/barré ; « H1 » en texte.
- **Ancrage droit de l'espaceur** (`refresh_menu`, appelé sur WM_SIZE) :
  1. somme des largeurs RÉELLES de tous les items via `GetMenuItemRect`
     (marges système incluses — les largeurs calculées localement sont trop
     petites d'~10 px/item et font déborder la ligne) ;
  2. espaceur = largeur cliente − somme − marge propre du système ;
  3. vérification adaptative : si un bouton droit est passé à la ligne
     suivante (`GetMenuItemRect.top` différent), on réduit l'espaceur de 40 px
     et on rebuild, jusqu'à tenir sur une ligne.
- Piège : `SetMenu` déclenche un `WM_SIZE` → **boucle infinie** WM_SIZE →
  SetMenu → WM_SIZE. Garde `IN_REFRESH` + `LAST_APPLIED` (on ne reconstruit
  que si la largeur calculée change de >2 px).
- Pièges windows 0.58 : `MEASUREITEMSTRUCT`/`DRAWITEMSTRUCT`/`ODT_MENU`/`ODS_*`
  vivent dans `Win32::UI::Controls` (et `ODS_FLAGS` est un tuple struct → test
  bit à bit `(itemState.0 & ODS_SELECTED.0) != 0`, pas de `contains`) ;
  `ClientToScreen` est dans `Win32::Graphics::Gdi` et renvoie un `BOOL`.
- Groupe gauche : `H1 ▾ | Listes ▾ | B | I | S̶ | Lien | Tableau ▾ | Mise en forme` ;
  groupe droit : `Outils d'écriture ▾ | Partager | Compte | ⚙` + séparateur central.
- Les boutons insèrent du **Markdown** comme le mode édition de l'original :
  B→`**`, I→`*`, S→`~~`, Lien→`[sel](https://)`, H1▾→préfixe `# `…`###### `/Normal,
  Listes▾→`- `/`1. `/`- [ ] `, Tableau▾→tableau 2/3/4 colonnes, Mise en forme→
  MAJUSCULES/minuscules/Capitaliser. Après wrap, la sélection couvre le texte
  sans les marqueurs (un 2ᵉ clic de mise en forme touche le texte).
  Partager=copie le chemin du fichier dans le presse-papiers ; ⚙→commentaires/
  à propos (sans lancer le Store).
- Bascule : menu Affichage → « Barre d'outils de mise en forme » (Ctrl+Shift+T),
  ID `cmd::TOOLBAR_TOGGLE=30042`, persistée dans `HKCU\Software\Microsoft\Notepad\
  IsFormattingToolbarVisible` (défaut 1 = visible) ; `rebuild_menus()` recrée la
  barre complète (les items owner-draw sont ajoutés seulement si visible).

## Vérifié bout en bout (verify.ps1, 10/10)

- `notepad.exe` (stub) lance bien `notepad-rs.exe` **à côté de lui** (pas le Store) ;
  aucun processus WindowsApps ne démarre ; le fichier passé en paramètre est chargé.
- Sans `notepad-rs.exe` : bannière `BannerClass` (fond crème, « Impossible de démarrer
  l'application Bloc-notes… », bouton « Obtenir le Bloc-notes ») ; ESC/0x102 la ferme,
  le stub se termine proprement. Le Store n'est ouvert que sur clic explicite du bouton.
- Autosave : texte tapé (WM_CHAR) + kill brutal → relance sans argument restaure le
  contenu (`probe.autosave` + fichiers `*.autosave` sous `%LOCALAPPDATA%\Microsoft\Notepad\Autosave`).
- `/P` ouvre la boîte d'impression (#32770) sans crash.

## Traces temporaires

- `cmdlog` (%TEMP%\nrs-cmd.log) **retiré** (app.rs, find.rs, dialogs.rs).
- Le panic-hook (%TEMP%\nrs-panic.log) est **conservé** : le sous-système GUI n'a pas
  de console, c'est le seul témoin en cas de panic.

## Comparatif original vs clone (mesuré 27/10)

| Métrique | Original Store 11.2607 | Clone Rust | Gain |
|---|---:|---:|---|
| Binaire éditeur | Notepad.exe 3,15 MB (+ riched20 3,86 MB, NotepadXamlUI 6,12 MB…) | notepad-rs.exe 0,38 MB | ~8× |
| Taille totale disque | paquet 18 MB (258 fichiers) | 0,64 MB (stub 267 Ko + éditeur 396 Ko) | ~28× |
| Lanceur | System32\notepad.exe 352 Ko | notepad.exe 267 Ko | ~1,3× |
| RAM (working set, doc vide) | 104 MB | 21 MB | ~5× |

Script de mesure : compare.ps1 (dossier des scripts de test).

## Lancement tests (rappel ; scripts dans le dossier des scripts de test)
```
build.ps1     # cargo +stable-x86_64-pc-windows-gnu build --release
func3.ps1     # 12/12 (fenêtre/onglets/find/goto/zoom/readonly)
tbtest3.ps1   # 4/4  (barre d'outils sur la ligne des menus, via WM_COMMAND 5001-5013)
tbtoggle.ps1  # 2/2  (Ctrl+Shift+T : masque/affiche les items owner-draw)
verify.ps1    # 10/10 (stub sans Store / bannière / autosave / /P)
compare.ps1   # comparatif tailles + RAM
```
Piège cross-process ajouté : `EM_EXSETSEL` avec pointeur `CHARRANGE` **non marshalled**
→ planque RichEdit côté cible ; utiliser `EM_SETSEL (0..len)` sans pointeur.
