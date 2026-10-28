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

### 开源、跨平台的 ManicTime 替代品

**自动记录时间，数据只留在你的电脑上。**<br>
在时间轴上看清一天的时间都去了哪里，把时间归到项目，生成真实的报告。免费且私密。

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/draugr-time/)
[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DraugrTime?style=social)](https://github.com/hlophlopgaming/DraugrTime/stargazers)

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · **简体中文** · [日本語](README.ja.md)

</div>

---



1.0 下载：[Releases](https://github.com/hlophlopgaming/DraugrTime/releases)。解压后，macOS 打开 `Draugr Time.app`，Windows/Linux 运行 `draugr-time`。Linux/macOS 命令行：`draugr-time --help`；Windows：`draugr-time-cli.exe`。[首次使用](docs/m10.md)。

## 为什么选择 Draugr Time？

早上 9 点打开电脑，一转眼就到了下午 6 点。这一天的时间都去哪儿了？

**ManicTime** 能很好地回答这个问题，但它是闭源软件，最有用的功能需要付费许可证。**Draugr Time** 想成为同类工具，并以开源方式开发：

- 🕒 **自动记录。** 记录当前应用、窗口标题以及你是否在电脑前。无需手动开始或停止计时器。
- 📊 **时间轴视图。** 一整天分布在并行的轨道上：电脑使用情况、应用、文档与网站、标签。
- 🏷️ **给时间打标签，而不是给任务。** 在时间轴上拖选一段时间，分配给某个项目或客户。规则可以自动完成这一步。
- 🔒 **默认私密。** 所有数据保存在本地的 SQLite 文件中。无需账号，没有云端，没有遥测，不发起任何网络请求。
- 💻 **真正跨平台。** 一套 Rust 代码支持 Windows、macOS 和 Linux，包括原生 **Wayland** 支持（GNOME、KDE、Sway/Hyprland），而不仅仅是 XWayland。
- 🆓 **永久免费。** GPL-3.0 许可。没有“Pro”版本。

## Draugr Time、ManicTime 与 ActivityWatch 对比

|  | **Draugr Time** | ManicTime | ActivityWatch |
|---|---|---|---|
| 开源 | ✅ GPL-3.0 | ❌ 专有软件 | ✅ MPL-2.0 |
| 价格 | ✅ 免费 | 免费版 + 付费 Pro 版 | ✅ 免费 |
| 数据保存在本机 | ✅ | ✅ | ✅ |
| 时间轴上手动标记时间段 | 🚧 计划中 (M5) | ✅ | 基于规则的分类 |
| 自动标签规则（应用 / 标题 / URL） | ✅ M6 | ✅ | ✅ |
| 原生 Wayland 后端 | ✅ GNOME/KDE/wlroots | — | 有限支持 |
| 从 ActivityWatch 导入 | ✅ | ❌ | — |

Draugr Time 的事件格式与 ActivityWatch 兼容，你可以把历史数据一起带过来。

## 路线图

- [x] **M0** Rust 工作区、Iced 桌面外壳、Windows / macOS / Linux 上的 CI
- [x] **M1** 带迁移的 SQLite 存储，以及心跳事件合并
- [x] **M2** 原生监视器：Windows、macOS、Linux X11 上的活动窗口与空闲时间
- [x] **M3** 后台守护进程、本地 API、离开检测 (AFK)、Wayland（GNOME、KDE、wlroots）
- [x] **M3.5** 应用框架：主窗口、导航、记录状态、带暂停/恢复的托盘图标
- [x] **M4** 日视图：时间轴、时间段选择、汇总面板（类似 ManicTime）
- [ ] **M5** 标签：在时间轴上选择时间段 → 分配项目
- [x] **M6** 自动标签规则、预览与撤销；八种界面语言 — [M6](docs/m6.md)
- [x] **M7** 报告、CSV/JSON 导出、从 ActivityWatch 导入
- [x] ~~**M9** 浏览器扩展~~ — 已取消：窗口标题已能记录浏览器页面
- [x] **M10 — 1.0** 全平台应用图标、Linux/macOS 内置 CLI、首次使用引导、基础隐私设置
- [ ] **M11 — 1.1** 安装包：Windows 安装程序、AppImage、Flatpak、macOS DMG
- [ ] **M12** 设计改进、备份、标题遮盖、备注、快捷键、文件路径、定时暂停“暂不记录…”、开机自启提示、ActivityWatch 分类导入/导出
- [ ] **以后** 截图、数据库加密、多设备同步

详细信息：[PLAN.md](PLAN.md) · 架构决策：[docs/decisions.md](docs/decisions.md)

## 试用（开发者）

目前还没有安装包。你可以从源码构建，并运行调试工具，它会每秒输出一次当前活动窗口和空闲时间：

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git draugr-time
cd draugr-time
cargo run --package draugr-time
```

构建要求和各平台说明：[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)（英文）。

## 隐私

时间记录工具能看到你做的一切，所以这一点绝不妥协：

- 所有数据都保存在本地的一个 SQLite 文件中，归你所有。
- Draugr Time **不发起任何外部网络请求**，**没有遥测**。
- 计划中：排除指定应用、隐藏窗口标题、可选的数据库加密。

## 常见问题

<details>
<summary><b>有没有免费、开源的 ManicTime 替代品？</b></summary>

这正是 Draugr Time 的目标：自动记录、时间轴和时间标签，免费并采用 GPL 许可，支持 Windows、macOS 和 Linux。
</details>

<details>
<summary><b>它支持 Linux 和 Wayland 吗？</b></summary>

支持，Linux 是一等公民平台。X11 已经可用。GNOME、KDE、Sway 和 Hyprland 的 Wayland 后端已实现。GNOME/KDE 需要本地集成，参见 [Wayland](docs/wayland.md)。仍需在实际环境中手动验证。
</details>

<details>
<summary><b>它也能替代 RescueTime、Timing 或 WakaTime 吗？</b></summary>

部分可以。和 RescueTime、Timing 一样，它会自动记录电脑使用情况，但所有数据都留在本地，也不需要订阅。与 WakaTime 不同，它并不局限于代码编辑器。
</details>

<details>
<summary><b>可以导入 ManicTime 或 ActivityWatch 的数据吗？</b></summary>

现已支持ActivityWatch导入，参见[M7](docs/m7.md)。ManicTime 导入正在考虑中：如果需要，请提交 issue。
</details>

<details>
<summary><b>它会把我的数据上传到任何地方吗？</b></summary>

不会。没有服务器、没有账号、没有数据分析。多设备同步以后可能会加入，但会是可选功能。
</details>

## 参与贡献

欢迎贡献，尤其欢迎熟悉以下领域的朋友：

- **Wayland** 合成器（GNOME Shell 扩展、KWin 脚本、wlroots IPC）
- **macOS** 辅助功能 (Accessibility) API
- **时间轴界面**设计与数据可视化

提交 issue 讨论想法，或从 [PLAN.md](PLAN.md) 中挑选一个里程碑。本项目借助 AI 编程代理开发，它们的工作规则写在 [AGENTS.md](AGENTS.md) 中。

如果你觉得 Draugr Time 有用，**点个 Star 能帮助更多人发现它。** ⭐

## 许可证

[GPL-3.0](LICENSE)

<sub>ManicTime、RescueTime、Timing 和 WakaTime 是其各自所有者的商标。Draugr Time 是独立项目，与它们没有任何关联。</sub>

[M7: Overview / Week / Reports / CLI](docs/m7.md).
