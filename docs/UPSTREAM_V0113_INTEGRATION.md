# v0.1.13 source integration | v0.1.13 源码整合

## Scope and source history

This fork integrates the official ElyOffice `v0.1.13` tag only. The fork base is `2db57bf367076d3bbfb1cde617ca41aa5ce20a41`; the peeled upstream tag commit is `38d4cc30dbffb3d6ecaa25f791b4aa510afaca3a`. The merge preserves two-parent Git history. The 31 upstream `main` commits after this tag are outside this integration.

The upstream project is maintained at [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice). Its v0.1.13 packages are upstream releases; this fork has not published a corresponding binary release or installer. The README and [ORIGIN.md](../ORIGIN.md) retain the historical source links, migration chain, original author credit, and this fork's maintainer attribution.

本 fork 仅整合 ElyOffice 官方 `v0.1.13` tag。fork 基线为 `2db57bf367076d3bbfb1cde617ca41aa5ce20a41`，上游 tag 剥离后的提交为 `38d4cc30dbffb3d6ecaa25f791b4aa510afaca3a`。本次保留真实双父 Git 合并历史；tag 之后上游 `main` 的 31 个提交不在本次范围内。

当前上游项目位于 [ELY-Open-Source/ElyOffice](https://github.com/ELY-Open-Source/ElyOffice)。v0.1.13 安装包属于上游发行物；本 fork 尚未发布对应的二进制版本或安装器。README 与 [ORIGIN.md](../ORIGIN.md) 保留历史源码地址、迁移链、原作者署名及本 fork 维护者归属。

## Product version and carried changes

The integrated source contains the v0.1.13 product structure and features, including ElyShow system media, ElyTable filter search, ElyMark cross-paragraph selection, document CLI operations, and signed updates. The fork's Windows LOK process supervision, startup and operation deadlines, blocked-write containment, strict completion-receipt checks, native-owner dispatch and callback lifetime protections, and installer path/preflight/rollback safeguards remain in the merged source.

Windows mail-attachment risk classification also has a small known-executable fallback for `.exe`, `.com`, `.bat`, and `.cmd`, combined with the system `AssocIsDangerous` result. This is a minimum set for the tested common executable types, not a complete file-execution policy.

The v0.1.13 LOK source still pins LibreOffice core `8221e31b3ac356a1623c672912a3d2b492f7e3d1`. Its Kit/Document class and method slots match the fork's patched runtime ABI; the tag's cursor-with-view feature uses the existing Kit feature slot and adds flag `1<<5`. The Windows owner hooks remain the fork's `ely_lok_owner_is_current_v1`, `ely_lok_owner_post_v1`, and `ely_lok_buffer_free_v1` contract. The upstream tag source does not establish whether a separately downloaded upstream Windows DLL exports those hooks, so this report makes no claim about that DLL's PE exports.

The PDF worker now owns its MuPDF shim build in `crates/ely-pdf-worker/build.rs`, including `ely_pen.c`. Optional `ELY_MUPDF_INCLUDE` and `ELY_OPENSSL_DIR` inputs allow the build to use the existing external Windows native dependency layout; the upstream default paths remain unchanged. No LibreOffice or MuPDF native rebuild was performed.

整合后的源码保留 v0.1.13 产品结构与功能，包括 ElyShow 系统媒体、ElyTable 筛选搜索、ElyMark 跨段选择、文档 CLI 操作和签名更新。fork 的 Windows LOK 进程监督、启动与操作期限、阻塞写入隔离、严格完成回执检查、native owner 调度与回调生命周期保护，以及安装器路径、预检和回滚防护均已保留。

Windows 邮件附件风险分类除系统 `AssocIsDangerous` 判断外，也对 `.exe`、`.com`、`.bat` 和 `.cmd` 设置小范围的已知可执行类型底线。这只涵盖本次验证的常见类型，并非完整的文件执行安全策略。

v0.1.13 LOK 源码仍固定 LibreOffice core `8221e31b3ac356a1623c672912a3d2b492f7e3d1`。Kit/Document 类与方法槽位和 fork 的补丁运行时 ABI 相符；tag 的 cursor-with-view 功能通过现有 Kit feature 槽位传入新增的 `1<<5` 标志。Windows owner hooks 继续使用 fork 的 `ely_lok_owner_is_current_v1`、`ely_lok_owner_post_v1` 和 `ely_lok_buffer_free_v1` 契约。上游 tag 源码无法证明另行下载的 Windows DLL 是否导出这些 hooks，因此本文不对该 DLL 的 PE 导出作结论。

PDF worker 的 MuPDF shim 构建现由 `crates/ely-pdf-worker/build.rs` 管理，其中包含 `ely_pen.c`。可选的 `ELY_MUPDF_INCLUDE` 与 `ELY_OPENSSL_DIR` 允许构建复用现有外部 Windows native 依赖目录，同时保留上游默认路径。本次没有重建 LibreOffice 或 MuPDF native runtime。

## Validation evidence

Validation used Rust `1.98.1-x86_64-pc-windows-msvc`, `--locked`, a D:-hosted Cargo target/cache/temp layout, the current-source worker binaries, and the existing patched runtime whose LibreOffice core pin matches v0.1.13.

| Check | Result |
| --- | --- |
| Windows workspace `cargo check --workspace --locked` | Passed for the eight apps and workspace crates. One existing `paint_tile` dead-code warning remains. |
| Windows `ely-platform` test suite | 92 passed. Covers the merged settings/directory behavior, Windows app opening/type routing, signature verification, and trust checks. |
| ElyMark current binary test suite | 179 passed. The real system-clipboard exercise was skipped by its explicit environment guard. |
| Fresh `ely-lok-worker.exe` build | Passed. Final SHA-256: `710B53A6573D8BAB5487210DC7524071E43667A54209CC7C8B1E32631194FC75`. |
| Fresh `ely-pdf-worker.exe` build and link | Passed against the existing MuPDF/OpenSSL static libraries. SHA-256: `49C6026C0ABD9DA97EA92B46200A79584D87DC173F90704CF9B5FC76777ECCFD`. |
| LOK supervisor tests | 7 passed. Covers Job assignment/bootstrap, normal and forced shutdown, blocked stdin, queue/deadline behavior, EOF failure, profile quarantine, child reap, and Job active-process count `0`. |
| LOK owner hook tests | 7 passed. Covers accepted/rejected posts, callback context reclamation, admission close, timeout/quarantine, and panic containment. |
| Strict Windows RESULT receipt tests | 7 passed. Covers alias classification, latching, duplicate/unknown/malformed/wrong receipts, and retained pending state on timeout. |
| Event parser tests | 11 passed, including both cursor JSON forms (`viewId` with relative geometry and `viewId` with absolute geometry). |
| External Stopper cancellation test | 1 passed. With a 30-second request timeout, the stuck load returned an error 7.1 ms after external cancellation; the OS process exited inside a separate five-second bound, then shutdown/reap confirmed Job active-process count `0` and a stale stopper no-op. |
| Real DOCX flow | Passed. Ran exact `text.replace` with `Quiet=true`, verified completion and an undo step, checked that a no-match request returns an error, saved/reopened the DOCX, and verified the replacement text. Shutdown/reap, Job `0`, profile lock, and temp cleanup assertions passed. |
| Real XLSX flow | Passed. Edited, saved, reopened, and verified the cell value; shutdown/reap and cleanup assertions passed. |
| Real PPTX flow | Passed. Edited, duplicated/hidden slides, saved, reopened, and verified; shutdown/reap and cleanup assertions passed. |
| Windows installer source harness | Production NSIS source compiled successfully; the harness using the current shared include and an automatically extracted `ElyBack` macro passed its preflight, no-mutation, reparse-point, and backup-restore cases. No product install or uninstall was run. |

The platform and ElyMark suites each contain explicit environment-gated system clipboard or printer checks. Those paths were not run: the test guard requires opt-in for replacing the system clipboard, and this machine has no duplex printer. Their test harnesses reported the skips; no system clipboard or printer behavior is claimed as verified.

验证使用 Rust `1.98.1-x86_64-pc-windows-msvc`、`--locked`、D 盘 Cargo target/cache/temp 布局、本次源码构建的 worker，以及 LibreOffice core 固定版本与 v0.1.13 相符的既有补丁运行时。

| 检查 | 结果 |
| --- | --- |
| Windows workspace `cargo check --workspace --locked` | 八个应用与 workspace crates 检查通过；保留一条既有 `paint_tile` dead-code 警告。 |
| Windows `ely-platform` 测试套件 | 92 项通过，覆盖合并后的设置/目录行为、Windows 应用打开/类型路由、签名验证及信任校验。 |
| ElyMark 当前二进制测试套件 | 179 项通过；真实系统 clipboard 测试由其显式环境 guard 跳过。 |
| 新构建 `ely-lok-worker.exe` | 通过。最终 SHA-256：`710B53A6573D8BAB5487210DC7524071E43667A54209CC7C8B1E32631194FC75`。 |
| 新构建并链接 `ely-pdf-worker.exe` | 使用已有 MuPDF/OpenSSL 静态库通过。SHA-256：`49C6026C0ABD9DA97EA92B46200A79584D87DC173F90704CF9B5FC76777ECCFD`。 |
| LOK supervisor 测试 | 7 项通过，覆盖 Job 分配/启动、正常与强制关闭、阻塞 stdin、队列/期限、EOF 失败、profile quarantine、子进程回收及 Job 活跃进程数为 `0`。 |
| LOK owner hook 测试 | 7 项通过，覆盖 post 接受/拒绝、回调上下文回收、停止接收、超时隔离及 panic containment。 |
| Windows 严格 RESULT 回执测试 | 7 项通过，覆盖 alias 分类、回执缓存、重复/未知/畸形/错误回执及超时后保留 pending。 |
| Event parser 测试 | 11 项通过，包括两类 cursor JSON（带 `viewId` 的相对几何与绝对几何）。 |
| 外部 Stopper 取消测试 | 1 项通过。请求期限设为 30 秒，外部取消后 7.1 毫秒内 stuck load 返回错误；独立五秒期限内 OS 进程退出，随后 shutdown/reap 确认 Job 活跃进程数 `0`，过期 stopper 不再操作。 |
| 真实 DOCX 流程 | 通过。执行 `Quiet=true` 的精确 `text.replace`，验证完成回执与 undo step；无匹配请求返回错误；保存、重开 DOCX 并核对替换文本。shutdown/reap、Job `0`、profile lock 与临时文件清理断言通过。 |
| 真实 XLSX 流程 | 通过。编辑、保存、重开并核对单元格值；shutdown/reap 与清理断言通过。 |
| 真实 PPTX 流程 | 通过。编辑、复制/隐藏幻灯片、保存、重开并核对；shutdown/reap 与清理断言通过。 |
| Windows 安装器源码 harness | 当前 NSIS 源码编译通过；使用当前共享 include、从当前源码自动提取的 `ElyBack` 宏进行的预检、未变更保护、reparse point 与备份恢复用例通过。本次没有运行产品安装或卸载。 |

平台与 ElyMark 套件都包含受环境变量保护的系统 clipboard 或打印机检查。本次没有运行这些路径：替换系统 clipboard 需要显式启用，而此机器没有可进行双面打印的打印机。测试 harness 记录了这些 skip；本文不声称已验证系统 clipboard 或打印机行为。

## Installer encoding and release limits

The installer path validator rejects unsafe rooted, traversal, device-name, and reparse-point entries before mutation. A separate source-level limitation remains: the generated installed-file manifest is UTF-8 without a BOM, while the NSIS reader and legacy marker writers use the system ANSI code page. A real UTF-8 Chinese path fixture (`中文\名.txt`) was read incorrectly by production `ElyNext` and failed closed. The harness's direct Unicode validator check is not a manifest round-trip test. This integration does not change marker encoding or claim Chinese manifest compatibility; a safe migration needs a complete encoding and legacy-marker design.

This fork has not published a v0.1.13 binary release or installer. The validation did not install, upgrade, uninstall, invoke UAC, or touch product registry/user directories. It did not rebuild LibreOffice, MuPDF, or other large native dependencies. The real office flows establish the tested LOK/runtime combination only; they do not establish compatibility with every upstream release DLL or every v0.1.13 feature.

安装器路径 validator 会在变更前拒绝 rooted、目录回退、设备名与 reparse point 条目。另有一项源码限制仍未解决：生成的已安装文件清单为无 BOM UTF-8，而 NSIS reader 和 legacy marker writer 使用系统 ANSI 代码页。真实 UTF-8 中文路径样本（`中文\名.txt`）经生产 `ElyNext` 读取后乱码并 fail-closed。harness 中直接调用 Unicode validator 不等于清单往返测试。本次没有更改 marker 编码，也不声称中文清单兼容；安全迁移需要完整设计新编码与旧 marker 兼容。

本 fork 尚未发布 v0.1.13 二进制版本或安装器。本次验证没有安装、升级、卸载产品，没有触发 UAC，也未访问产品注册表或用户目录；没有重建 LibreOffice、MuPDF 或其他大型 native 依赖。真实办公文档流程只验证了本次测试的 LOK/runtime 组合，不代表兼容所有上游 release DLL 或覆盖 v0.1.13 的全部功能。

## License notices

The fork retains its baseline AGPL-3.0-or-later `LICENSE` and Cargo workspace/package license metadata. The merged upstream v0.1.13 MIT-licensed source and its notices retain their upstream provenance. The tag's complete MIT notice, including `Copyright (c) 2026 Yang Zhang`, is preserved verbatim as `LICENSE-UPSTREAM-MIT`; source-file notices remain intact. The two notices identify their respective provenance and do not present the entire merged repository as a work users may choose to license under either one. The app license collection script now includes both notices.

本 fork 保留基线 AGPL-3.0-or-later `LICENSE` 及 Cargo workspace/package 许可元数据。本次合并的上游 v0.1.13 MIT 来源代码及其 notice 保留原有来源。tag 所带完整 MIT notice（含 `Copyright (c) 2026 Yang Zhang`）逐字保存在 `LICENSE-UPSTREAM-MIT`；源文件原有许可说明保持不变。这两份 notice 分别标明其来源，不表示整个合并仓库可由使用者任选其中任一许可。应用包许可收集脚本现在会同时收录两份 notice。
