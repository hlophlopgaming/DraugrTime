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

**Локальный учёт активности с нативным приложением на Rust.**

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/draugr-time/)
[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust + Iced](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)

[English](README.md) · **Русский** · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

Draugr Time записывает активное приложение, заголовок окна и время активности
или отсутствия, затем показывает их на дневном таймлайне. Это бесплатный
проект под GPL-3.0, вдохновлённый рабочим процессом ManicTime. История остаётся
на вашем компьютере.



Готовые архивы 1.0: [Releases](https://github.com/hlophlopgaming/DraugrTime/releases). Распакуйте архив; на macOS откройте `Draugr Time.app`, на Windows/Linux — `draugr-time`. На Linux/macOS CLI встроен: `draugr-time --help`; на Windows — `draugr-time-cli.exe`. [Первый запуск](docs/m10.md).

## Что работает

- Нативный интерфейс **Iced** со светлой, тёмной и системной темами.
- Отслеживание активного окна и простоя на Windows, macOS и Linux X11.
- Бэкенды Wayland для GNOME, KDE, Sway и Hyprland. Для GNOME/KDE нужен локальный
  мост композитора: [настройка Wayland](docs/wayland.md).
- Дневной таймлайн: состояние компьютера, приложения, документы и теги;
  масштабирование, прокрутка, выделение диапазона и сводка длительности.
  Иерархические теги, заметки, редактирование и undo доступны.
- Фоновая запись, управление из трея, пауза и автозапуск по желанию.
- Обзор дня, недельный вид, месячные отчёты и табель; CSV/JSON, импорт ActivityWatch
  и отдельная консоль `draugr-time-cli`.
- Локальная SQLite-база и выбор папки данных.

Один исполняемый файл содержит интерфейс, трекинг, хранилище и localhost API.
Веб-фронтенда нет, отдельный сервис запускать не нужно. В обычном запуске
демо-режим отсутствует.

## Скриншоты

На скриншотах показаны сгенерированные примеры, а не личная история активности.

![Draugr Time — светлая тема](docs/screenshots/draugr-time-light.png)

![Draugr Time — тёмная тема](docs/screenshots/draugr-time-dark.png)

## Сборка и запуск

Установите стабильный Rust. На Linux нужны пакеты разработки librsvg и libxkbcommon-x11,
панель с поддержкой StatusNotifierItem и xdg-desktop-portal с бэкендом рабочего стола. На Windows нужен MSVC, на macOS —
Xcode command-line tools. Готовых установщиков пока нет.

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git
cd DraugrTime
cargo run --release --package draugr-time
```

Исполняемый файл: `target/release/draugr-time` (`.exe` на Windows).
Нажмите **Start recording**, чтобы начать запись. Закрытие окна оставляет
приложение и запись в трее; **Quit** завершает работу.

[Инструкции для разработки](docs/DEVELOPMENT.md) описывают проверки и диагностику,
а [документация приложения](docs/desktop.md) — настройки и особенности платформ.

## Данные и приватность

- История хранится локально в SQLite. Нет аккаунта, облачной отправки и телеметрии.
- Сетевые обращения приложения ограничены localhost; IPC композитора тоже локальный.
- Новые установки используют системную папку данных `DraugrTime`. Существующая
  история может продолжать храниться в прежней папке `DagdaTime`.
- Settings → Data folder выбирает папку для следующего запуска. Существующая
  история остаётся на прежнем месте.

## Дорожная карта

- [x] **M0–M3.5:** workspace, хранилище, нативные коллекторы, локальный API и оболочка.
- [x] **M4:** дневной таймлайн, выделение диапазона и сводка.
- [ ] **M5:** редактирование тегов, менеджер тегов и разметка времени отсутствия.
- [x] **M6:** автотеги с предпросмотром и отменой, восемь языков, определение языка ОС и сохранение выбора. [Проверки M6](docs/m6.md).
- [x] **M7:** недельный вид, отчёты, экспорт CSV/JSON и импорт ActivityWatch.
- [x] ~~**M9:** расширение браузера~~ — отменено: заголовков окон браузера достаточно.
- [x] **M10 — 1.0:** иконка приложения везде, встроенный CLI на Linux/macOS, первый запуск, базовые настройки приватности.
- [ ] **M11 — 1.1:** установщики: Windows, AppImage, Flatpak, DMG для macOS.
- [ ] **M12:** дизайн, резервные копии, маскирование заголовков, заметки, горячие клавиши, пути файлов, паузы «Не записывать…» по таймеру, предложение автозапуска, импорт/экспорт категорий ActivityWatch.

Подробности: [PLAN.md](PLAN.md) · [Архитектурные решения](docs/decisions.md).

## Участие в разработке

Обсудите изменение в [issue](https://github.com/hlophlopgaming/DraugrTime/issues).
Особенно полезна помощь с разрешениями ОС, интеграциями Wayland и проверкой
на реальных рабочих столах. Инструкции для агентов: [AGENTS.md](AGENTS.md).

## Лицензия

[GNU GPLv3](LICENSE). Draugr Time — независимый проект, не связанный с ManicTime
или ActivityWatch.

M6 добавляет автотеги по регулярным выражениям, приоритеты, предпросмотр и
отменяемое применение к истории. В настройках доступны английский, русский,
немецкий, французский, испанский, португальский, упрощённый китайский и японский
или язык системы. Подробнее: [M6](docs/m6.md).

Обзор дня, недельные таймлайны, отчёты и локальная консоль: [инструкция M7](docs/m7.md).
