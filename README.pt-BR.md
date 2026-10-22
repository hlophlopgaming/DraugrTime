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

### Alternativa de código aberto e multiplataforma ao ManicTime

**Controle de tempo automático que fica no seu computador.**<br>
Veja em uma linha do tempo para onde foi o seu dia, atribua tempo a projetos e tenha relatórios honestos. Gratuito e privado.

[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DraugrTime?style=social)](https://github.com/hlophlopgaming/DraugrTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · **Português** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---



Arquivos 1.0: [Releases](https://github.com/hlophlopgaming/DraugrTime/releases). Extraia e abra `Draugr Time.app` no macOS ou `draugr-time` no Windows/Linux. CLI Linux/macOS: `draugr-time --help`; Windows: `draugr-time-cli.exe`. [Primeiro uso](docs/m10.md).

## Por que o Draugr Time?

Você abre o notebook às 9h e, de repente, já são 18h. Para onde foi o dia?

O **ManicTime** responde bem a essa pergunta, mas tem código fechado e os recursos mais úteis exigem uma licença paga. O **Draugr Time** quer ser o mesmo tipo de ferramenta, desenvolvida de forma aberta:

- 🕒 **Rastreamento automático.** Registra o aplicativo ativo, o título da janela e se você estava no computador. Sem cronômetros para iniciar ou esquecer.
- 📊 **Linha do tempo.** O seu dia inteiro em trilhas paralelas: uso do computador, aplicativos, documentos e sites, tags.
- 🏷️ **Marque o tempo, não as tarefas.** Selecione um trecho na linha do tempo e atribua a um projeto ou cliente. Regras podem fazer isso automaticamente.
- 🔒 **Privado por padrão.** Tudo fica em um arquivo SQLite local. Sem conta, sem nuvem, sem telemetria, sem chamadas de rede.
- 💻 **Multiplataforma de verdade.** Windows, macOS e Linux a partir de uma única base de código em Rust, com suporte nativo a **Wayland** (GNOME, KDE, Sway/Hyprland), não apenas XWayland.
- 🆓 **Gratuito para sempre.** GPL-3.0. Sem versão "Pro".

## Draugr Time, ManicTime e ActivityWatch

|  | **Draugr Time** | ManicTime | ActivityWatch |
|---|---|---|---|
| Código aberto | ✅ GPL-3.0 | ❌ Proprietário | ✅ MPL-2.0 |
| Preço | ✅ Gratuito | Edição gratuita, Pro paga | ✅ Gratuito |
| Os dados ficam na sua máquina | ✅ | ✅ | ✅ |
| Linha do tempo com marcação manual de intervalos | 🚧 Planejado (M5) | ✅ | Categorias por regras |
| Regras de marcação automática (app / título / URL) | ✅ M6 | ✅ | ✅ |
| Backends nativos para Wayland | ✅ GNOME/KDE/wlroots | — | Limitado |
| Importação do ActivityWatch | ✅ | ❌ | — |

O formato de eventos do Draugr Time é compatível com o ActivityWatch, então você poderá trazer o seu histórico.

## Roadmap

- [x] **M0** Workspace Rust, aplicativo desktop com Iced, CI no Windows / macOS / Linux
- [x] **M1** Armazenamento SQLite com migrações e mesclagem de eventos heartbeat
- [x] **M2** Watchers nativos: janela ativa e tempo ocioso no Windows, macOS, Linux X11
- [x] **M3** Serviço em segundo plano, API local, detecção de ausência (AFK), Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** Estrutura do aplicativo: janela principal, navegação, status do rastreamento, ícone na bandeja com pausar/retomar
- [x] **M4** Visão do dia: linha do tempo, seleção de intervalos, painel de resumo (como no ManicTime)
- [ ] **M5** Tags: selecionar um trecho na linha do tempo → atribuir um projeto
- [x] **M6** Regras automáticas, prévia e desfazer; oito idiomas da interface — [M6](docs/m6.md)
- [x] **M7** Relatórios, exportação CSV/JSON, importação do ActivityWatch
- [x] ~~**M9** Extensão de navegador~~ — descartada: os títulos das janelas já cobrem as páginas
- [x] **M10 — 1.0** Ícone do app em todo lugar, CLI integrada no Linux/macOS, primeiro uso, privacidade básica
- [ ] **M11 — 1.1** Instaladores: Windows, AppImage, Flatpak, DMG para macOS
- [ ] **M12** Design, backups, ocultar títulos, notas, atalhos, caminhos de arquivo, pausas com tempo «Não registrar por…», pergunta de inicialização automática, importação/exportação de categorias do ActivityWatch
- [ ] **Depois** Capturas de tela, criptografia do banco de dados, sincronização entre dispositivos

Detalhes: [PLAN.md](PLAN.md) · Decisões de arquitetura: [docs/decisions.md](docs/decisions.md)

## Experimente (desenvolvedores)

Ainda não há instaladores. Você pode compilar a partir do código-fonte e rodar a ferramenta de depuração, que mostra a cada segundo a janela ativa e o tempo ocioso:

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git draugr-time
cd draugr-time
cargo run --package draugr-time
```

Requisitos de compilação e notas por plataforma: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (em inglês).

## Privacidade

Um rastreador de tempo vê tudo o que você faz, então aqui não há concessões:

- Todos os dados são armazenados localmente em um único arquivo SQLite que pertence a você.
- O Draugr Time **não faz chamadas de rede externas** e **não tem telemetria**.
- Planejado: excluir aplicativos, ocultar títulos de janelas, criptografia opcional do banco de dados.

## Perguntas frequentes

<details>
<summary><b>Existe uma alternativa gratuita e de código aberto ao ManicTime?</b></summary>

É exatamente isso que o Draugr Time quer ser: rastreamento automático, linha do tempo e marcação de tempo, gratuito e sob licença GPL, no Windows, macOS e Linux.
</details>

<details>
<summary><b>Funciona no Linux e no Wayland?</b></summary>

Sim, o Linux é uma plataforma de primeira classe. O X11 já funciona. Os backends Wayland para GNOME, KDE, Sway e Hyprland estão implementados. GNOME/KDE exigem integração local; veja [Wayland](docs/wayland.md). A verificação manual continua necessária.
</details>

<details>
<summary><b>Também é uma alternativa ao RescueTime, Timing ou WakaTime?</b></summary>

Em parte. Como o RescueTime e o Timing, ele registra automaticamente o uso do computador, mas tudo fica local e sem assinatura. Diferente do WakaTime, não se limita a editores de código.
</details>

<details>
<summary><b>Posso importar meus dados do ManicTime ou do ActivityWatch?</b></summary>

A importação do ActivityWatch está disponível; veja [M7](docs/m7.md). A importação do ManicTime está em avaliação: abra uma issue se precisar dela.
</details>

<details>
<summary><b>Ele envia meus dados para algum lugar?</b></summary>

Não. Não há servidor, conta nem analytics. A sincronização entre dispositivos pode vir depois, e será opcional.
</details>

## Contribuindo

Contribuições são bem-vindas, especialmente de quem conhece:

- compositores **Wayland** (extensões do GNOME Shell, scripts do KWin, IPC do wlroots);
- as APIs de acessibilidade do **macOS**;
- design de **interfaces de linha do tempo** e visualização de dados.

Abra uma issue para discutir uma ideia ou escolha um marco do [PLAN.md](PLAN.md). O projeto é desenvolvido com agentes de programação com IA; as regras de trabalho deles estão em [AGENTS.md](AGENTS.md).

Se o Draugr Time parece útil para você, **uma estrela ajuda outras pessoas a encontrá-lo.** ⭐

## Licença

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing e WakaTime são marcas de seus respectivos proprietários. O Draugr Time é um projeto independente e não tem vínculo com eles.</sub>

[M7: Overview / Week / Reports / CLI](docs/m7.md).
