<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo/whiteDraugr128.png">
  <img src="logo/blackDraugr128.png" alt="Draugr Time" width="96" height="96">
</picture>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo/whiteCrown128.png">
  <img src="logo/blackCrown128.png" alt="Draugr Time tray crown" width="32" height="32">
</picture>

# Draugr Time

### Alternative open source et multiplateforme à ManicTime

**Un suivi du temps automatique qui reste sur votre ordinateur.**<br>
Voyez où est passée votre journée sur une timeline, associez votre temps à des projets et obtenez des rapports fiables. Gratuit et privé.

[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DraugrTime?style=social)](https://github.com/hlophlopgaming/DraugrTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · **Français** · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---



Archives 1.0 : [Releases](https://github.com/hlophlopgaming/DraugrTime/releases). Extrayez puis lancez `Draugr Time.app` sur macOS, `draugr-time` sur Windows/Linux. CLI Linux/macOS : `draugr-time --help` ; Windows : `draugr-time-cli.exe`. [Premier lancement](docs/m10.md).

## Pourquoi Draugr Time ?

Vous ouvrez votre ordinateur à 9 h, et soudain il est 18 h. Où est passée la journée ?

**ManicTime** répond bien à cette question, mais son code est fermé et ses fonctions les plus utiles exigent une licence payante. **Draugr Time** veut être le même type d'outil, développé en open source :

- 🕒 **Suivi automatique.** Enregistre l'application active, le titre de la fenêtre et votre présence devant l'ordinateur. Aucun chronomètre à lancer ou à oublier.
- 📊 **Timeline.** Toute votre journée sur des pistes parallèles : utilisation de l'ordinateur, applications, documents et sites web, étiquettes.
- 🏷️ **Étiquetez le temps, pas les tâches.** Sélectionnez une plage sur la timeline et attribuez-la à un projet ou un client. Des règles peuvent le faire automatiquement.
- 🔒 **Privé par défaut.** Tout est stocké dans un fichier SQLite local. Pas de compte, pas de cloud, pas de télémétrie, aucun appel réseau.
- 💻 **Vraiment multiplateforme.** Windows, macOS et Linux à partir d'une seule base de code Rust, avec une prise en charge native de **Wayland** (GNOME, KDE, Sway/Hyprland), pas seulement XWayland.
- 🆓 **Gratuit pour toujours.** GPL-3.0. Pas de version « Pro ».

## Draugr Time, ManicTime et ActivityWatch

|  | **Draugr Time** | ManicTime | ActivityWatch |
|---|---|---|---|
| Open source | ✅ GPL-3.0 | ❌ Propriétaire | ✅ MPL-2.0 |
| Prix | ✅ Gratuit | Édition gratuite, Pro payante | ✅ Gratuit |
| Les données restent sur votre machine | ✅ | ✅ | ✅ |
| Timeline avec étiquetage manuel de plages horaires | 🚧 Prévu (M5) | ✅ | Catégories par règles |
| Règles d'étiquetage automatique (app / titre / URL) | ✅ M6 | ✅ | ✅ |
| Backends Wayland natifs | ✅ GNOME/KDE/wlroots | — | Limité |
| Import depuis ActivityWatch | ✅ | ❌ | — |

Le format d'événements de Draugr Time est compatible avec ActivityWatch : vous pourrez emporter votre historique.

## Feuille de route

- [x] **M0** Workspace Rust, interface de bureau Iced, CI sous Windows / macOS / Linux
- [x] **M1** Stockage SQLite avec migrations et fusion des événements heartbeat
- [x] **M2** Watchers natifs : fenêtre active et temps d'inactivité sous Windows, macOS, Linux X11
- [x] **M3** Service en arrière-plan, API locale, détection d'absence (AFK), Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** Squelette de l'application : fenêtre principale, navigation, état du suivi, icône dans la barre système avec pause/reprise
- [x] **M4** Vue Jour : timeline, sélection de plages, panneau récapitulatif (comme dans ManicTime)
- [ ] **M5** Étiquettes : sélectionner une plage sur la timeline → attribuer un projet
- [x] **M6** Règles automatiques, aperçu et annulation ; huit langues d’interface — [M6](docs/m6.md)
- [x] **M7** Rapports, export CSV/JSON, import depuis ActivityWatch
- [x] ~~**M9** Extension de navigateur~~ — abandonnée : les titres de fenêtre couvrent déjà les pages
- [x] **M10 — 1.0** Icône partout, CLI intégrée sous Linux/macOS, premier lancement, confidentialité de base
- [ ] **M11 — 1.1** Installateurs : Windows, AppImage, Flatpak, DMG macOS
- [ ] **M12** Design, sauvegardes, masquage des titres, notes, raccourcis, chemins de fichiers, pauses minutées « Ne pas suivre pendant… », proposition de démarrage automatique, import/export des catégories ActivityWatch
- [ ] **Plus tard** Captures d'écran, chiffrement de la base, synchronisation entre appareils

Détails : [PLAN.md](PLAN.md) · Décisions d'architecture : [docs/decisions.md](docs/decisions.md)

## Essayer (développeurs)

Il n'y a pas encore d'installateur. Vous pouvez compiler depuis les sources et lancer l'outil de débogage, qui affiche chaque seconde la fenêtre active et le temps d'inactivité :

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git draugr-time
cd draugr-time
cargo run --package draugr-time
```

Prérequis de compilation et notes par plateforme : [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (en anglais).

## Confidentialité

Un outil de suivi du temps voit tout ce que vous faites, donc aucun compromis ici :

- Toutes les données sont stockées localement dans un seul fichier SQLite qui vous appartient.
- Draugr Time n'effectue **aucun appel réseau externe** et ne collecte **aucune télémétrie**.
- Prévu : exclusion d'applications, masquage des titres de fenêtres, chiffrement optionnel de la base.

## FAQ

<details>
<summary><b>Existe-t-il une alternative gratuite et open source à ManicTime ?</b></summary>

C'est exactement l'objectif de Draugr Time : suivi automatique, timeline et étiquetage du temps, gratuit et sous licence GPL, sous Windows, macOS et Linux.
</details>

<details>
<summary><b>Fonctionne-t-il sous Linux et Wayland ?</b></summary>

Oui, Linux est une plateforme à part entière. X11 fonctionne déjà. Les backends Wayland GNOME, KDE, Sway et Hyprland sont implémentés. GNOME/KDE nécessitent une intégration locale ; voir [Wayland](docs/wayland.md). Une vérification manuelle reste nécessaire.
</details>

<details>
<summary><b>Est-ce aussi une alternative à RescueTime, Timing ou WakaTime ?</b></summary>

En partie. Comme RescueTime et Timing, il suit automatiquement votre utilisation de l'ordinateur, mais tout reste en local et sans abonnement. Contrairement à WakaTime, il ne se limite pas aux éditeurs de code.
</details>

<details>
<summary><b>Puis-je importer mes données ManicTime ou ActivityWatch ?</b></summary>

L’import ActivityWatch est disponible ; voir [M7](docs/m7.md). L'import depuis ManicTime est à l'étude : ouvrez une issue si vous en avez besoin.
</details>

<details>
<summary><b>Mes données sont-elles envoyées quelque part ?</b></summary>

Non. Pas de serveur, pas de compte, pas d'analytics. Une synchronisation entre appareils pourra arriver plus tard, et elle sera optionnelle.
</details>

## Contribuer

Les contributions sont les bienvenues, en particulier de la part de personnes qui connaissent :

- les compositeurs **Wayland** (extensions GNOME Shell, scripts KWin, IPC wlroots) ;
- les API d'accessibilité de **macOS** ;
- le design d'**interfaces de timeline** et la visualisation de données.

Ouvrez une issue pour discuter d'une idée, ou choisissez une étape dans [PLAN.md](PLAN.md). Le projet est développé avec des agents de programmation IA ; leurs règles de travail sont dans [AGENTS.md](AGENTS.md).

Si Draugr Time vous semble utile, **une étoile aide d'autres personnes à le trouver.** ⭐

## Licence

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing et WakaTime sont des marques de leurs propriétaires respectifs. Draugr Time est un projet indépendant, sans lien avec eux.</sub>

[M7: Overview / Week / Reports / CLI](docs/m7.md).
