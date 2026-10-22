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

### Open-Source- und plattformübergreifende Alternative zu ManicTime

**Automatische Zeiterfassung, die auf deinem Computer bleibt.**<br>
Sieh auf einer Zeitleiste, wo dein Tag geblieben ist, ordne Zeit Projekten zu und erhalte ehrliche Berichte. Kostenlos und privat.

[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DraugrTime?style=social)](https://github.com/hlophlopgaming/DraugrTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · **Deutsch** · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---



Archive für 1.0: [Releases](https://github.com/hlophlopgaming/DraugrTime/releases). Entpacken, dann unter macOS `Draugr Time.app`, unter Windows/Linux `draugr-time` starten. Linux/macOS: `draugr-time --help`; Windows: `draugr-time-cli.exe`. [Erster Start](docs/m10.md).

## Warum Draugr Time?

Du klappst um 9 Uhr den Laptop auf, und plötzlich ist es 18 Uhr. Wo ist der Tag geblieben?

**ManicTime** beantwortet diese Frage gut, ist aber Closed Source, und die nützlichsten Funktionen gibt es nur mit einer kostenpflichtigen Lizenz. **Draugr Time** will dieselbe Art von Werkzeug sein, offen entwickelt:

- 🕒 **Automatische Erfassung.** Zeichnet die aktive App, den Fenstertitel und deine Anwesenheit am Computer auf. Keine Timer, die man starten oder vergessen kann.
- 📊 **Zeitleiste.** Dein ganzer Tag auf parallelen Spuren: Computernutzung, Anwendungen, Dokumente und Websites, Tags.
- 🏷️ **Zeit taggen, nicht Aufgaben.** Markiere einen Bereich auf der Zeitleiste und weise ihn einem Projekt oder Kunden zu. Regeln können das automatisch erledigen.
- 🔒 **Standardmäßig privat.** Alles liegt in einer lokalen SQLite-Datei. Kein Konto, keine Cloud, keine Telemetrie, keine Netzwerkzugriffe.
- 💻 **Wirklich plattformübergreifend.** Windows, macOS und Linux aus einer Rust-Codebasis, inklusive nativer **Wayland**-Unterstützung (GNOME, KDE, Sway/Hyprland), nicht nur XWayland.
- 🆓 **Für immer kostenlos.** GPL-3.0. Keine „Pro“-Version.

## Draugr Time, ManicTime und ActivityWatch im Vergleich

|  | **Draugr Time** | ManicTime | ActivityWatch |
|---|---|---|---|
| Open Source | ✅ GPL-3.0 | ❌ Proprietär | ✅ MPL-2.0 |
| Preis | ✅ Kostenlos | Kostenlose Edition, kostenpflichtiges Pro | ✅ Kostenlos |
| Daten bleiben auf deinem Rechner | ✅ | ✅ | ✅ |
| Zeitleiste mit manuellem Taggen von Zeitbereichen | 🚧 Geplant (M5) | ✅ | Regelbasierte Kategorien |
| Auto-Tagging-Regeln (App / Titel / URL) | ✅ M6 | ✅ | ✅ |
| Native Wayland-Backends | ✅ GNOME/KDE/wlroots | — | Eingeschränkt |
| Import aus ActivityWatch | ✅ | ❌ | — |

Das Ereignisformat von Draugr Time ist mit ActivityWatch kompatibel, sodass du deine Historie mitnehmen kannst.

## Roadmap

- [x] **M0** Rust-Workspace, Iced-Desktop-Shell, CI unter Windows / macOS / Linux
- [x] **M1** SQLite-Speicher mit Migrationen und Zusammenführung von Heartbeat-Ereignissen
- [x] **M2** Native Watcher: aktives Fenster und Leerlaufzeit unter Windows, macOS, Linux X11
- [x] **M3** Hintergrunddienst, lokale API, Abwesenheitserkennung (AFK), Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** App-Grundgerüst: Hauptfenster, Navigation, Tracking-Status, Tray-Symbol mit Pause/Fortsetzen
- [x] **M4** Tagesansicht: Zeitleiste, Bereichsauswahl, Zusammenfassung (wie in ManicTime)
- [ ] **M5** Tags: Bereich auf der Zeitleiste markieren → Projekt zuweisen
- [x] **M6** Auto-Tagging-Regeln, Vorschau und Rückgängig; acht Oberflächensprachen — [M6](docs/m6.md)
- [x] **M7** Berichte, CSV/JSON-Export, Import aus ActivityWatch
- [x] ~~**M9** Browser-Erweiterung~~ – gestrichen: Fenstertitel decken Browserseiten bereits ab
- [x] **M10 – 1.0** App-Symbol überall, integrierte CLI unter Linux/macOS, Einführung, grundlegende Datenschutzeinstellungen
- [ ] **M11 – 1.1** Installationspakete: Windows-Installer, AppImage, Flatpak, macOS-DMG
- [ ] **M12** Design, Backups, Titelmaskierung, Notizen, Tastenkürzel, Dateipfade, zeitgesteuerte Pausen „Nicht erfassen für…“, Autostart-Abfrage, Import/Export von ActivityWatch-Kategorien
- [ ] **Später** Screenshots, Datenbankverschlüsselung, Synchronisierung zwischen Geräten

Details: [PLAN.md](PLAN.md) · Architekturentscheidungen: [docs/decisions.md](docs/decisions.md)

## Ausprobieren (für Entwickler)

Installer gibt es noch nicht. Du kannst aus dem Quellcode bauen und das Debug-Werkzeug starten, das einmal pro Sekunde das aktive Fenster und die Leerlaufzeit ausgibt:

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git draugr-time
cd draugr-time
cargo run --package draugr-time
```

Build-Voraussetzungen und Plattformhinweise: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (auf Englisch).

## Datenschutz

Ein Zeiterfassungs-Tool sieht alles, was du tust. Deshalb gibt es hier keine Kompromisse:

- Alle Daten werden lokal in einer einzigen SQLite-Datei gespeichert, die dir gehört.
- Draugr Time macht **keine externen Netzwerkzugriffe** und sammelt **keine Telemetrie**.
- Geplant: Apps ausschließen, Fenstertitel maskieren, optionale Datenbankverschlüsselung.

## FAQ

<details>
<summary><b>Gibt es eine kostenlose Open-Source-Alternative zu ManicTime?</b></summary>

Genau das soll Draugr Time werden: automatische Erfassung, Zeitleiste und Taggen von Zeit, kostenlos und unter GPL, für Windows, macOS und Linux.
</details>

<details>
<summary><b>Funktioniert es unter Linux und Wayland?</b></summary>

Ja, Linux ist eine vollwertige Plattform. X11 funktioniert bereits. Wayland-Backends für GNOME, KDE, Sway und Hyprland sind implementiert. GNOME/KDE benötigen eine lokale Integration; siehe [Wayland](docs/wayland.md). Manuelle Prüfung bleibt erforderlich.
</details>

<details>
<summary><b>Ist es auch eine Alternative zu RescueTime, Timing oder WakaTime?</b></summary>

Teilweise. Wie RescueTime und Timing erfasst es deine Computernutzung automatisch, aber alles bleibt lokal und es gibt kein Abo. Anders als WakaTime ist es nicht auf Code-Editoren beschränkt.
</details>

<details>
<summary><b>Kann ich meine Daten aus ManicTime oder ActivityWatch importieren?</b></summary>

Der Import aus ActivityWatch ist verfügbar; siehe [M7](docs/m7.md). Ein ManicTime-Import wird geprüft: Eröffne ein Issue, wenn du ihn brauchst.
</details>

<details>
<summary><b>Werden meine Daten irgendwohin hochgeladen?</b></summary>

Nein. Es gibt keinen Server, kein Konto und keine Analytics. Eine Synchronisierung zwischen Geräten kommt vielleicht später, und sie wird optional sein.
</details>

## Mitmachen

Beiträge sind willkommen, besonders von Leuten, die sich auskennen mit:

- **Wayland**-Compositoren (GNOME-Shell-Erweiterungen, KWin-Skripte, wlroots-IPC)
- den Accessibility-APIs von **macOS**
- **Zeitleisten-UI**-Design und Datenvisualisierung

Eröffne ein Issue, um eine Idee zu besprechen, oder nimm dir einen Meilenstein aus [PLAN.md](PLAN.md). Das Projekt wird mit KI-Coding-Agenten entwickelt; ihre Arbeitsregeln stehen in [AGENTS.md](AGENTS.md).

Wenn dir Draugr Time nützlich erscheint, **hilft ein Stern anderen, es zu finden.** ⭐

## Lizenz

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing und WakaTime sind Marken ihrer jeweiligen Inhaber. Draugr Time ist ein unabhängiges Projekt und steht in keiner Verbindung zu ihnen.</sub>

[M7: Overview / Week / Reports / CLI](docs/m7.md).
