# ElyOffice

ElyOffice 是一套面向本地文件的开源桌面办公应用，涵盖文档、表格、演示、PDF、Git 仓库管理和 Markdown 编辑。本仓库维护其 Windows 派生版本。文件保存在你的设备上；日常处理本地文件无需账号或云端存储。

ElyOffice is an open-source desktop suite for working with local files, covering documents, spreadsheets, presentations, PDFs, Git repository management, and Markdown editing. This repository maintains its Windows derivative. Your files stay on your device; working with local files does not require an account or cloud storage.

## 应用 | Apps

| 应用 | 用途 | Purpose |
| --- | --- | --- |
| ElyDoc | 编辑文档 | Write and edit documents |
| ElyTable | 处理表格与 CSV 文件 | Work with spreadsheets and CSV files |
| ElyShow | 制作演示文稿 | Create presentations |
| ElyPDF | 阅读与处理 PDF | Read and work with PDFs |
| ElyTree | 管理 Git 仓库 | Manage Git repositories |
| ElyMark | 编辑 Markdown | Edit Markdown |

## 获取 ElyOffice | Get ElyOffice

本仓库目前提供源码，尚未发布 Windows 安装包或二进制版本。请在 [GitHub Releases](https://github.com/mmlong818/ElyOffice/releases) 查看发布状态。

This repository currently provides source code; no Windows installer or binary release has been published. Check [GitHub Releases](https://github.com/mmlong818/ElyOffice/releases) for release status.

## 开发 | Development

ElyOffice 使用 Rust 与 GPUI 构建界面。ElyDoc、ElyTable 和 ElyShow 通过 LibreOfficeKit 使用 LibreOffice 文档引擎；ElyPDF 使用 MuPDF 和 Tesseract。Windows 构建条件与当前限制见 [Windows 整合说明](docs/WINDOWS_INTEGRATION.md)。贡献入口见 [CONTRIBUTING.md](CONTRIBUTING.md)。

ElyOffice uses Rust and GPUI for its interface. ElyDoc, ElyTable, and ElyShow use the LibreOffice document engine through LibreOfficeKit; ElyPDF uses MuPDF and Tesseract. See the [Windows integration notes](docs/WINDOWS_INTEGRATION.md) for Windows build requirements and current limitations. See [CONTRIBUTING.md](CONTRIBUTING.md) to get started contributing.

## 来源与许可 | Credits and licence

ElyOffice 由 [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) 创建（[X](https://x.com/ZacharyZhang)）。原仓库为 [github.com/ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice)，现已迁移至 [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)。本 fork 位于 [github.com/mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。文档引擎来自 [LibreOffice 社区](https://zh-cn.libreoffice.org/)开发的 LibreOffice。

ElyOffice was created by [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) ([X](https://x.com/ZacharyZhang)). The original repository is [github.com/ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice), now migrated to [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice). This fork is [github.com/mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`. The document engine comes from LibreOffice, developed by the [LibreOffice community](https://www.libreoffice.org/).

项目沿革与详细归属见[来源说明](ORIGIN.md)。

See [ORIGIN.md](ORIGIN.md) for project history and detailed attribution.

自有代码采用 AGPL-3.0-or-later，见 [LICENSE](LICENSE)。第三方组件各自采用其许可，详见[许可与源码说明](docs/licenses.md)。

ElyOffice code is licensed under AGPL-3.0-or-later; see [LICENSE](LICENSE). Third-party components retain their respective licences; see [licence and source notes](docs/licenses.md).
