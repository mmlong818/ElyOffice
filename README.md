# ElyOffice

ElyOffice 是一套面向本地文件的开源桌面应用，主要包含文档、表格、演示文稿、PDF、Git 和 Markdown 工具。文件保存在你的设备上，处理本地文件无需账号或云端存储。

ElyOffice is an open-source desktop suite for local files, with document, spreadsheet, presentation, PDF, Git, and Markdown apps. Local-file work stays on your device and does not require an account or cloud storage.

## Apps | 应用

| App | Purpose | Engine |
| --- | --- | --- |
| ElyDoc | Word processing | LibreOfficeKit |
| ElyTable | Spreadsheets and CSV | LibreOfficeKit |
| ElyShow | Presentations and media | LibreOfficeKit, system media |
| ElyPDF | PDF reading and editing, OCR, signatures | MuPDF, Tesseract |
| ElyMail | Mail over IMAP, POP3 and SMTP | Rust mail engine, SQLite |
| ElyTree | Git repository management | Git |
| ElyMark | Markdown editing, math and diagrams | GPUI, markdown-rs |
| ElyAgent | AI assistant | Kigi CLI |

## Get ElyOffice | 获取 ElyOffice

This fork currently provides source code and has no binary release or installer. See [this fork's Releases](https://github.com/mmlong818/ElyOffice/releases). The original project publishes builds from its [v0.1.13 release](https://github.com/ELY-Open-Source/ElyOffice/releases/tag/v0.1.13); those are upstream packages and do not include this fork's Windows worker and installer changes.

本 fork 当前提供源码，尚未发布二进制版本或安装包。请查看[本 fork 的 Releases](https://github.com/mmlong818/ElyOffice/releases)。原项目在 [v0.1.13 release](https://github.com/ELY-Open-Source/ElyOffice/releases/tag/v0.1.13) 发布的安装包属于上游版本，不包含本 fork 的 Windows worker 与安装器改动。

This source follows the original project's `v0.1.13` tag. It includes that version's product features, including ElyShow system media, ElyTable filter search, ElyMark cross-paragraph selection, document CLI operations, and signed updates, together with the fork's Windows stability and installer protections. See the [integration report](docs/UPSTREAM_V0113_INTEGRATION.md) for the verified scope and limitations.

本仓库源码跟进原项目 `v0.1.13` tag，保留该版本的产品功能，包括 ElyShow 系统媒体、ElyTable 筛选搜索、ElyMark 跨段选择、文档 CLI 操作和签名更新，并整合本 fork 的 Windows 稳定性与安装器保护。已验证范围和限制见[整合报告](docs/UPSTREAM_V0113_INTEGRATION.md)。

## Development | 开发

The interface uses Rust and GPUI. ElyDoc, ElyTable, and ElyShow use LibreOffice through LibreOfficeKit; ElyPDF uses MuPDF and Tesseract. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and the [integration report](docs/UPSTREAM_V0113_INTEGRATION.md) for Windows build and runtime notes.

界面使用 Rust 与 GPUI。ElyDoc、ElyTable 和 ElyShow 通过 LibreOfficeKit 使用 LibreOffice；ElyPDF 使用 MuPDF 与 Tesseract。开发环境见 [CONTRIBUTING.md](CONTRIBUTING.md)，Windows 构建与运行记录见[整合报告](docs/UPSTREAM_V0113_INTEGRATION.md)。

## Credits and licence | 来源与许可

ElyOffice was created by [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) ([X](https://x.com/ZacharyZhang)). The original source moved from [its historical GitHub repository](https://github.com/ZacharyZhang-NY/ElyOffice) to [the author's Gitea repository](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice), then to the current [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice) GitHub project. This public fork is [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`. LibreOffice and LibreOfficeKit are developed by the [LibreOffice community](https://www.libreoffice.org/). See [ORIGIN.md](ORIGIN.md) for project history and attribution.

ElyOffice 由 [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) 创建（[X](https://x.com/ZacharyZhang)）。原始源码先由[历史 GitHub 仓库](https://github.com/ZacharyZhang-NY/ElyOffice)迁至[作者的 Gitea 仓库](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)，随后迁至当前的 [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice) GitHub 项目。本公开 fork 位于 [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。LibreOffice 与 LibreOfficeKit 由 [LibreOffice 社区](https://www.libreoffice.org/)开发。项目沿革与归属见 [ORIGIN.md](ORIGIN.md)。

本 fork 沿用 [AGPL-3.0-or-later](LICENSE)。合入的上游 v0.1.13 代码保留其 MIT 版权与许可声明，全文见 [LICENSE-UPSTREAM-MIT](LICENSE-UPSTREAM-MIT)。第三方组件适用各自许可，详见[许可与来源说明](docs/licenses.md)。

This fork continues under [AGPL-3.0-or-later](LICENSE). The merged upstream v0.1.13 code retains its MIT copyright and license notice in [LICENSE-UPSTREAM-MIT](LICENSE-UPSTREAM-MIT). Third-party components retain their respective licences; see the [licence and source notes](docs/licenses.md).
