# 来源与贡献归属 | Origin and attribution

本项目源自 ElyOffice。原 GitHub 仓库地址为 [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice)，本次准备时返回 HTTP 404。原作者迁移后的上游仓库为 [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)。本派生公开仓库为 [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。

This project originates from ElyOffice. Its former GitHub repository URL was [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice), which returned HTTP 404 during this preparation. The original author's migrated upstream repository is [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice). This derivative public repository is [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`.

LibreOffice 及 LibreOfficeKit 由 [LibreOffice 社区](https://zh-cn.libreoffice.org/)开发；参见 [LibreOfficeKit 文档](https://docs.libreoffice.org/libreofficekit.html)。原 ElyOffice 项目及其界面与集成由 ZacharyZhang-NY 和贡献者开发。本 Windows 公开派生版本由 `mmlong818` 维护。以上说明用于标明项目和来源归属，不表示存在官方隶属或认可。

LibreOffice and LibreOfficeKit are developed by the [LibreOffice community](https://www.libreoffice.org/); see the [LibreOfficeKit documentation](https://docs.libreoffice.org/libreofficekit.html). The original ElyOffice project, its interface, and its integration were developed by ZacharyZhang-NY and contributors. This public Windows derivative is maintained by `mmlong818`. These are project and source attributions; they make no claim of official affiliation or endorsement.

| 角色 | 仓库 / 账号 | 说明 |
| --- | --- | --- |
| 原项目旧地址 | [GitHub: ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice) | 本次准备时返回 HTTP 404 |
| 原作者迁移上游 | [git.zacharyzhang.com: ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | 原作者项目的上游仓库 |
| 本派生仓库 | [GitHub: mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice) | 维护者 `mmlong818` |
| 原作者账号 | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | 上游提交显示名为“雷电芽衣” |
| 原作者 X | [x.com/ZacharyZhang](https://x.com/ZacharyZhang) | 原作者 X |

| Role | Repository / account | Description |
| --- | --- | --- |
| Former original URL | [GitHub: ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice) | Returned HTTP 404 during this preparation |
| Original author's migrated upstream | [git.zacharyzhang.com: ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | Upstream repository for the original project |
| This derivative repository | [GitHub: mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice) | Maintainer: `mmlong818` |
| Original author account | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | Upstream commit display name: “雷电芽衣” |
| Original author X | [x.com/ZacharyZhang](https://x.com/ZacharyZhang) | Original author X |

本仓库保留原作者已有的署名、版权及许可声明；这些声明不表示版权转让，也不改变原作者权利。`mmlong818` 维护的 fork 工作及其他新增贡献，应按各自实际贡献者署名，不归称为原作者原创。仓库根目录的原始 [LICENSE](LICENSE) 保持原文，项目许可声明为 AGPL-3.0-or-later。第三方许可及声明见 [docs/licenses.md](docs/licenses.md)。

This repository retains the original author's existing attribution, copyright, and license notices. These notices do not imply a copyright transfer or change the original author's rights. Fork maintenance by `mmlong818` and other new contributions should be attributed to their respective contributors, not presented as original work by the upstream author. The original root [LICENSE](LICENSE) is retained verbatim; the project declares AGPL-3.0-or-later. Third-party licenses and notices are listed in [docs/licenses.md](docs/licenses.md).

## 基线与上游整合 | Baseline and upstream integration

早期 fork 基线为 `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`。先前 Windows 源码提交为 `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`；它保留了原作者历史。当前整合树将迁移上游 `main` 截止快照 `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb`（2026-10-11）合入本 fork，并保留两边历史。该上游快照比早期 `ff92f26` 多 210 个提交。本次整合报告记录检查范围与限制：[docs/WINDOWS_INTEGRATION.md](docs/WINDOWS_INTEGRATION.md)。

The early fork baseline is `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`. The previous Windows source commit is `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`; it preserves the original author history. The current integration tree merges migrated upstream `main` at the reviewed cutoff snapshot `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb` (2026-10-11) into this fork while retaining both histories. That upstream snapshot is 210 commits beyond the early `ff92f26` baseline. The current integration report records checks and limits: [docs/WINDOWS_INTEGRATION.md](docs/WINDOWS_INTEGRATION.md).

上游提交 [09d2a6b68132314df3555e87f97971a5ddce9088](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/commit/09d2a6b68132314df3555e87f97971a5ddce9088) 已为八个应用加入 release GUI 子系统属性，并让 LOK/PDF worker 以无控制台模式启动。因此，相同行为不是本 fork 独有或领先上游的功能。

Upstream commit [09d2a6b68132314df3555e87f97971a5ddce9088](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/commit/09d2a6b68132314df3555e87f97971a5ddce9088) already adds release GUI subsystem attributes to all eight apps and starts LOK/PDF workers without a console. Matching behavior is not unique to this fork or ahead of upstream.

## Windows 工作范围 | Windows work in this fork

本 fork 的 Windows 工作包括构建和安装脚本可移植性、LOK worker Job Object 监督与 bootstrap/deadline、Windows 路径及进程处理，以及打包脚本。上游更新已整合到当前工作树；旧安装程序（2026-10-11）仍只对应 `59cb32d`，不包含本次整合源码，也未在本轮重建。

Windows work in this fork includes build and installer portability, LOK worker Job Object supervision with bootstrap and deadlines, Windows path and process handling, and packaging scripts. Upstream updates are integrated into the current source tree; the earlier installer (2026-10-11) still corresponds only to `59cb32d`, does not contain this integration, and was not rebuilt this round.

套件安装需要调用方提供 CEF/PDF worker stages、LibreOffice runtime 与许可文件、LOK 应用二进制、Cargo release 目录、Git for Windows 和 NSIS。仓库脚本不会在干净 checkout 中自动生成所有预制 stage 与本地 readiness 元数据。本次没有执行完整 native CEF、LibreOffice 或 MuPDF 重建，也没有运行整套安装器。

Suite installation requires caller-provided CEF/PDF worker stages, LibreOffice runtime and license files, LOK app binaries, a Cargo release directory, Git for Windows, and NSIS. Repository scripts do not automatically generate every prebuilt stage and local readiness metadata from a clean checkout. This round did not rebuild native CEF, LibreOffice, or MuPDF in full and did not run the complete installer.

LibreOffice 默认跨平台补丁链保留 `0001`–`0039`。Windows SAL 补丁 [`0053-sal-long-path-threshold-boundary.patch`](patches/windows/libreoffice/0053-sal-long-path-threshold-boundary.patch) 独立保存，SHA-256 为 `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`，不属于默认补丁链。可用来源材料没有 `0040`–`0052` 的补丁原件或完整原生 Windows LibreOffice 构建步骤，因此本仓库不声称可从现有内容完整重建 Windows LibreOffice runtime。细节见 [patches/windows/libreoffice/README.md](patches/windows/libreoffice/README.md)。

The default cross-platform LibreOffice patch chain retains `0001`–`0039`. Windows SAL patch [`0053-sal-long-path-threshold-boundary.patch`](patches/windows/libreoffice/0053-sal-long-path-threshold-boundary.patch) is kept separately; its SHA-256 is `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`. It is outside the default patch chain. The available provenance does not include original patches `0040`–`0052` or a complete native Windows LibreOffice build procedure, so this repository does not claim that the complete Windows LibreOffice runtime can be rebuilt from its current contents. See [patches/windows/libreoffice/README.md](patches/windows/libreoffice/README.md) for details.

脚本生成的 Windows 安装程序默认未签名。本仓库此次准备的是源码；不包含 exe、安装包或二进制 release。发布安装程序时，应单独核对其来源、完整对应源码、第三方许可和签名流程。

Windows installers produced by the scripts are unsigned by default. This preparation is for source; it includes no exe, installer, or binary release. Before distributing an installer, verify its provenance, complete corresponding source, third-party licenses, and signing process separately.
