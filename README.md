# ElyOffice

ElyOffice 是一套面向本地文件的开源桌面办公应用，提供文档、表格、演示文稿、PDF、Git 仓库管理和 Markdown 编辑。本仓库维护其 Windows 派生版本，当前源码跟进原作者 v0.1.8 系列。文件保存在你的设备上，处理本地文件无需账号或云端存储。

ElyOffice is an open-source desktop suite for local files, with apps for documents, spreadsheets, presentations, PDFs, Git repositories, and Markdown. This repository maintains a Windows derivative whose source follows the original author's v0.1.8 line. Your files stay on your device; working with local files requires no account or cloud storage.

## 应用 | Apps

| 应用 | 用途 | Purpose |
| --- | --- | --- |
| ElyDoc | 编辑文档，改进 Word 公式与行内图片显示 | Edit documents, with improved rendering of Word formulas and inline images |
| ElyTable | 处理表格与 CSV；工作表较多时可横向滚动标签 | Work with spreadsheets and CSV; scroll sheet tabs horizontally when there are many |
| ElyShow | 制作演示文稿 | Create presentations |
| ElyPDF | 阅读与处理 PDF | Read and work with PDFs |
| ElyTree | 管理 Git 仓库 | Manage Git repositories |
| ElyMark | 原生 Markdown 编辑器，支持行内与块级数学公式 | Native Markdown editor with inline and block math |

## 获取 ElyOffice | Get ElyOffice

本仓库目前提供源码，尚未发布可下载的 Windows 安装包或二进制版本。请查看[本仓库 Releases](https://github.com/mmlong818/ElyOffice/releases)。如需 Windows 安装包，可访问[原作者 v0.1.8 下载页](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/releases/tag/v0.1.8)，其中提供 Windows 套件及六个单应用包。这些安装包来自原作者版本，不包含本 fork 额外的稳定性与安装器修复。

This repository currently provides source code and has not published a downloadable Windows installer or binary release. Check [this fork's Releases](https://github.com/mmlong818/ElyOffice/releases). For Windows installers, visit [the original author's v0.1.8 downloads](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/releases/tag/v0.1.8), which include the suite and six standalone app packages. Those are the original author's packages and do not include the additional stability and installer fixes maintained in this fork.

## 开发 | Development

界面使用 Rust 与 GPUI 构建；ElyDoc、ElyTable 和 ElyShow 通过 LibreOfficeKit 使用 LibreOffice 文档引擎，ElyPDF 使用 MuPDF 与 Tesseract。Windows 构建与打包现状见[上游 v0.1.8 整合报告](docs/UPSTREAM_V018_INTEGRATION.md)。贡献入口见 [CONTRIBUTING.md](CONTRIBUTING.md)。

The interface is built with Rust and GPUI. ElyDoc, ElyTable, and ElyShow use the LibreOffice document engine through LibreOfficeKit; ElyPDF uses MuPDF and Tesseract. See the [upstream v0.1.8 integration report](docs/UPSTREAM_V018_INTEGRATION.md) for the current Windows build and packaging status. See [CONTRIBUTING.md](CONTRIBUTING.md) to get started contributing.

## 来源与许可 | Credits and licence

ElyOffice 由 [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) 创建（[X](https://x.com/ZacharyZhang)）。原仓库为 [github.com/ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice)，现已迁移至 [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)。本 fork 位于 [github.com/mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。文档引擎来自 [LibreOffice 社区](https://zh-cn.libreoffice.org/)开发的 LibreOffice。项目沿革与详细归属见[来源说明](ORIGIN.md)。

ElyOffice was created by [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) ([X](https://x.com/ZacharyZhang)). The original repository is [github.com/ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice), now migrated to [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice). This fork is [github.com/mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`. The document engine comes from LibreOffice, developed by the [LibreOffice community](https://zh-cn.libreoffice.org/). See [ORIGIN.md](ORIGIN.md) for project history and detailed attribution.

自有代码采用 AGPL-3.0-or-later，见 [LICENSE](LICENSE)。第三方组件各自采用其许可，详见[许可与源码说明](docs/licenses.md)。

ElyOffice code is licensed under AGPL-3.0-or-later; see [LICENSE](LICENSE). Third-party components retain their respective licences; see [licence and source notes](docs/licenses.md).
