# 来源与贡献归属 | Origin and attribution

ElyOffice 由 [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) 创建。原始 GitHub 仓库 [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice) 是项目历史来源；随后代码与发行物迁至作者的 [Gitea 仓库](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)。该 Gitea 项目说明代码和发行物现已迁至 GitHub，当前原作者项目位于 [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice)。本公开派生仓库为 [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。

ElyOffice was created by [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY). The historical source is the original GitHub repository [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice); the code and releases later moved to the author's [Gitea repository](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice). That Gitea project says the code and releases have moved to GitHub. The current original project is [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice). This public derivative is [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`.

LibreOffice and LibreOfficeKit are developed by the [LibreOffice community](https://www.libreoffice.org/); see the [LibreOfficeKit documentation](https://docs.libreoffice.org/libreofficekit.html). ElyOffice and its interface and integration were developed by ZacharyZhang-NY and contributors. The fork's Windows maintenance and additional changes are maintained by `mmlong818`. These statements identify project history and sources; they do not imply official affiliation or endorsement.

LibreOffice 及 LibreOfficeKit 由 [LibreOffice 社区](https://zh-cn.libreoffice.org/)开发；参见 [LibreOfficeKit 文档](https://docs.libreoffice.org/libreofficekit.html)。ElyOffice 及其界面、集成由 ZacharyZhang-NY 和贡献者开发。本 fork 的 Windows 维护和新增改动由 `mmlong818` 维护。以上内容用于说明项目沿革与来源，不表示存在官方隶属或认可。

| Role | Repository / account | Description |
| --- | --- | --- |
| Original creator | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | Creator of ElyOffice; also [X](https://x.com/ZacharyZhang) |
| Historical source | [GitHub: ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice) | Original source repository |
| Previous migration location | [Gitea: ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | Historical repository; its notice points to the current GitHub project |
| Current original project | [GitHub: ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice) | Current upstream source and releases |
| Public fork | [GitHub: mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice) | Maintained by `mmlong818` |

| 角色 | 仓库 / 账号 | 说明 |
| --- | --- | --- |
| 原作者 | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | ElyOffice 创建者；亦见 [X](https://x.com/ZacharyZhang) |
| 历史源码仓库 | [GitHub: ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice) | 项目原始 GitHub 源码地址 |
| 之前的迁移地址 | [Gitea: ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | 历史仓库；站内说明指向当前 GitHub 项目 |
| 当前原作者项目 | [GitHub: ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice) | 当前上游源码与发行地址 |
| 本公开 fork | [GitHub: mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice) | 维护者 `mmlong818` |

本仓库保留原作者已有的署名、版权及许可声明；这些声明不表示版权转让，也不改变原作者权利。`mmlong818` 维护的 fork 工作及其他新增贡献，应按各自实际贡献者署名，不归称为原作者原创。仓库根目录原有 AGPL [LICENSE](LICENSE) 保持原文，fork 的 Cargo 许可元数据也继续标示 AGPL-3.0-or-later。本次合并带入的上游 v0.1.13 MIT 来源代码及其 notice 保留原有来源；tag 根目录所带的完整 MIT notice（含 `Copyright (c) 2026 Yang Zhang`）以原文字节保留于 [LICENSE-UPSTREAM-MIT](LICENSE-UPSTREAM-MIT)，单独列示且不替换 fork 的 AGPL 文本。第三方许可及声明见 [docs/licenses.md](docs/licenses.md)。

This repository retains the original author's existing attribution, copyright, and license notices. These notices do not imply a copyright transfer or change the original author's rights. Fork maintenance by `mmlong818` and other new contributions should be attributed to their respective contributors, not presented as original work by the upstream author. The fork's original root AGPL [LICENSE](LICENSE) and Cargo license metadata remain unchanged. The merged upstream v0.1.13 MIT-licensed source and its notices retain their upstream provenance; the tag's full MIT notice, including `Copyright (c) 2026 Yang Zhang`, is preserved byte-for-byte in [LICENSE-UPSTREAM-MIT](LICENSE-UPSTREAM-MIT), separately from the fork's AGPL text. Third-party licenses and notices are listed in [docs/licenses.md](docs/licenses.md).

## 基线与上游整合 | Baseline and upstream integration

早期 fork 基线为 `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`。此前 Windows 源码提交 `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d` 和 v0.1.8 整合记录均为历史状态，仍保留原作者历史。本次源码从公开 fork `origin/main` 的 `2db57bf367076d3bbfb1cde617ca41aa5ce20a41` 开始，以真实双父 Git 合并整合当前上游正式 `v0.1.13` tag，其提交为 `38d4cc30dbffb3d6ecaa25f791b4aa510afaca3a`。不包含 v0.1.13 之后上游 main 的 31 个提交。验证范围和限制见[本次 v0.1.13 整合报告](docs/UPSTREAM_V0113_INTEGRATION.md)；先前的 [v0.1.8 整合报告](docs/UPSTREAM_V018_INTEGRATION.md) 与 [Windows 整合说明](docs/WINDOWS_INTEGRATION.md) 保持为历史记录。

The early fork baseline is `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`. The previous Windows source commit `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d` and the v0.1.8 integration record are historical snapshots; original author history remains intact. This source integration starts from the public fork's `origin/main` at `2db57bf367076d3bbfb1cde617ca41aa5ce20a41` and merges the current official upstream `v0.1.13` tag at commit `38d4cc30dbffb3d6ecaa25f791b4aa510afaca3a` with real two-parent Git history. It excludes the 31 upstream main commits after v0.1.13. Validation scope and limitations are in the [v0.1.13 integration report](docs/UPSTREAM_V0113_INTEGRATION.md); the earlier [v0.1.8 integration report](docs/UPSTREAM_V018_INTEGRATION.md) and [Windows integration notes](docs/WINDOWS_INTEGRATION.md) remain historical records.

Windows work in this fork includes build and installer portability, LibreOfficeKit worker Job Object supervision and deadlines, Windows path and process handling, and packaging protections. This integration keeps the upstream v0.1.13 application and worker structure while carrying forward fork-specific safeguards where they remain compatible.

本 fork 的 Windows 改动包括构建与安装器可移植性、LibreOfficeKit worker Job Object 监督与期限、Windows 路径和进程处理及打包保护。本次整合保留 v0.1.13 应用与 worker 结构，并在兼容处延续 fork 的安全保护。
