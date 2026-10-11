# Windows 与上游整合验收 | Windows and upstream integration report

本报告描述源码整合工作树截至 2026-10-11 的上游截止快照，以及在 Windows x64 MSVC 环境完成的检查。它不表示已经创建新的合并提交或发布新的安装包。

This report describes the source integration worktree at its upstream cutoff snapshot dated 2026-10-11 and checks performed in a Windows x64 MSVC environment. It does not mean that a new merge commit or installer has been published.

## 来源与版本 | Provenance and revisions

| 项目 | 中文说明 | English |
| --- | --- | --- |
| 原 GitHub 仓库 | [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice)，本次准备时返回 HTTP 404。 | [ZacharyZhang-NY/ElyOffice](https://github.com/ZacharyZhang-NY/ElyOffice), which returned HTTP 404 during preparation. |
| 迁移后的原作者上游 | [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice)。 | [git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice). |
| 本派生仓库 | [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice)，由 `mmlong818` 维护。 | [mmlong818/ElyOffice](https://github.com/mmlong818/ElyOffice), maintained by `mmlong818`. |
| 原作者 | 账号 [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY)，上游提交显示名“雷电芽衣”；[原作者 X](https://x.com/ZacharyZhang)。 | Account [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY), upstream commit display name “雷电芽衣”; [original author X](https://x.com/ZacharyZhang). |

早期 fork 基线是 `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`；原作者历史之后的先前 Windows 发布源码提交是 `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`。本次整合采用原作者迁移上游 `main` 的审阅截止提交 `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb`（2026-10-11，标题：`macOS CI: build from the dev clone's engines, never from scratch past the 3h runner limit (L1.8)`）。该快照比早期基线领先 210 个提交。整合方式为保留 fork 与上游提交历史的普通合并；截止快照之后的上游新提交不在本轮范围内。

The early fork baseline is `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`; the previous Windows source release commit after the original author history is `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`. This integration uses the reviewed cutoff commit `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb` from the original author's migrated upstream `main` (2026-10-11, title: `macOS CI: build from the dev clone's engines, never from scratch past the 3h runner limit (L1.8)`). This snapshot is 210 commits ahead of the early baseline. The integration preserves both the fork and upstream histories through a normal merge; upstream commits after this cutoff are outside this round.

## 本 fork 的 Windows 工作 | Windows work in this fork

本 fork 在上游已有的无控制台启动支持基础上，增加 Windows worker 的 Job Object 生命周期监督、bootstrap 与 deadline 处理、请求/结果关联、Windows 安装与构建脚本，以及独立保存的 SAL 补丁。上游已有的 8 个应用 release GUI 子系统设置及 LOK/PDF `CREATE_NO_WINDOW` 行为属于上游改进，不归为本 fork 原创。Windows LOK worker 的请求处理保持单一串行 reader；它在独立请求线程处理请求和等待结果，让 LibreOffice owner loop 继续泵事件。teardown 通过唯一 CAS claim 避免并发清理；意外 owner loop 退出时采用 quarantine/fail-fast，由父 supervisor 回收 Job。

Building on upstream's existing no-console startup support, this fork adds Windows worker Job Object lifecycle supervision, bootstrap and deadline handling, request/result correlation, Windows build and installer scripts, and a separately preserved SAL patch. Upstream's release GUI subsystem settings in all eight apps and LOK/PDF `CREATE_NO_WINDOW` behavior are upstream work, not claimed as original to this fork. The Windows LOK worker keeps a single serial reader for requests and result waits on a separate request thread, allowing the LibreOffice owner loop to keep pumping events. Teardown uses a single CAS claim to prevent concurrent cleanup; an unexpected owner-loop exit quarantines and fails fast so the parent supervisor can reap the Job.

该监督边界不意味着每个 native 调用都受 Condvar 的 30 秒等待限制；若 native `post` 或 `joinThreads` 本身阻塞，最终硬停止上限由父 supervisor 的 Job 监管提供。LibreOffice 补丁 `0053-sal-long-path-threshold-boundary.patch` 的 SHA-256 为 `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`。当前来源材料仍缺少 `0040`–`0052` 的原始 patch 文件及完整 Windows native LibreOffice 构建流程；0053 单独保存且不加入默认跨平台 patch 链。详见 [补丁说明](../patches/windows/libreoffice/README.md)。

This supervision boundary does not mean every native call is bounded by a 30-second Condvar wait. If a native `post` or `joinThreads` call itself blocks, the final hard-stop limit is provided by the parent supervisor's Job control. The SHA-256 of `0053-sal-long-path-threshold-boundary.patch` is `66FA79185CB10E911FB23934CEB4BC76505BB7E0F74F7AEE1E0B010C146D8DD7`. The available provenance still lacks original patch files `0040`–`0052` and a complete Windows native LibreOffice build procedure; 0053 is stored separately and is not part of the default cross-platform patch chain. See the [patch notes](../patches/windows/libreoffice/README.md).

## 验证结果 | Validation results

验证使用 D 盘隔离的 Cargo/Rust 工具链与缓存、Windows x64 MSVC host target，以及已有的 SAL-r2 runtime stage。执行了锁文件约束的 8 个应用 `cargo check --locked`；检查通过，Cargo.lock 未更新。`ely-lok` 的 supervisor fixture 7/7 通过，RESULT receipt 测试 7/7 通过。最终无临时诊断代码的 worker 已重建，SHA-256 为 `0119F4E98CF5C7DF1EF12EDF2F923D179B4C51936EF56B819C0506F1E9546C15`。基于该 worker 的真实 XLSX 流程 1/1 通过：打开、编辑、保存、关闭、重新打开并正常关闭；日志记录线程 join 和 teardown 成功。最终源码回归日志与 8-app 检查分别位于 `final-source-regression-01` 和 `app-check-online-final` 任务输出目录（这些本机日志未随源码提交）。

Validation used the isolated D-drive Cargo/Rust toolchain and cache, the Windows x64 MSVC host target, and the existing SAL-r2 runtime stage. The eight-app `cargo check --locked` completed successfully without updating Cargo.lock. The `ely-lok` supervisor fixtures passed 7/7, and RESULT receipt tests passed 7/7. The final worker was rebuilt without temporary diagnostic code; its SHA-256 is `0119F4E98CF5C7DF1EF12EDF2F923D179B4C51936EF56B819C0506F1E9546C15`. A real XLSX flow using that worker passed 1/1: open, edit, save, close, reopen, and normal shutdown; the log records successful thread join and teardown. Final-source regression logs and the eight-app check are in the `final-source-regression-01` and `app-check-online-final` task output directories (these local logs are not included in the source submission).

本轮没有从头构建 LibreOffice、CEF 或 MuPDF，没有运行完整安装器，也没有生成或发布新二进制安装包。此前在 2026-10-11 验证的安装程序仍只对应源码提交 `59cb32d`，不包含此次上游整合。脚本默认生成未签名安装程序；原始根目录 [LICENSE](../LICENSE) 保持原文，项目声明 AGPL-3.0-or-later。

This round did not build LibreOffice, CEF, or MuPDF from scratch, run the full installer, or produce or publish a new binary installer. The installer previously verified on 2026-10-11 still corresponds only to source commit `59cb32d` and does not include this upstream integration. The scripts produce unsigned installers by default; the original root [LICENSE](../LICENSE) remains verbatim, and the project declares AGPL-3.0-or-later.
