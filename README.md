# Ely Office

ElyOffice 是采用 Rust／GPUI 界面的第三方办公套件；本仓库维护其 Windows 派生版本。ElyDoc、ElyTable 和 ElyShow 通过 LibreOfficeKit 使用 LibreOffice 文档引擎；ElyPDF 使用 MuPDF 与 Tesseract。离线优先，文件保存在你的文件夹中；无需账号或云服务。

ElyOffice is a third-party office suite with a Rust/GPUI interface. This repository maintains its Windows derivative. ElyDoc, ElyTable, and ElyShow use the LibreOffice document engine through LibreOfficeKit; ElyPDF uses MuPDF and Tesseract. The apps are offline-first; files stay in your folders, with no account or cloud service required.

| 项目角色 | 仓库 | 说明 |
| --- | --- | --- |
| 旧原仓库地址 | [ZacharyZhang-NY/ElyOffice on GitHub](https://github.com/ZacharyZhang-NY/ElyOffice) | 本次核查时返回 HTTP 404 |
| 迁移后的上游仓库 | [ZacharyZhang-NY/ElyOffice on git.zacharyzhang.com](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | 原项目上游 |
| 本 fork | [mmlong818/ElyOffice on GitHub](https://github.com/mmlong818/ElyOffice) | 由 `mmlong818` 维护 |
| 原作者账号 | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | 原提交显示名为“雷电芽衣” |
| 原作者 X | [x.com/ZacharyZhang](https://x.com/ZacharyZhang) | 原作者 X |

| Project role | Repository | Description |
| --- | --- | --- |
| Former original repository URL | [ZacharyZhang-NY/ElyOffice on GitHub](https://github.com/ZacharyZhang-NY/ElyOffice) | Returned HTTP 404 during this review |
| Migrated upstream repository | [ZacharyZhang-NY/ElyOffice on git.zacharyzhang.com](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice) | Upstream for the original project |
| This fork | [mmlong818/ElyOffice on GitHub](https://github.com/mmlong818/ElyOffice) | Maintained by `mmlong818` |
| Original author account | [ZacharyZhang-NY](https://github.com/ZacharyZhang-NY) | Original commit display name: “雷电芽衣” |
| Original author X | [x.com/ZacharyZhang](https://x.com/ZacharyZhang) | Original author X |

原作者的署名、版权和许可声明按原样保留。本 fork 的维护及新增贡献按各自贡献者署名，不将其归于原作者。许可为 AGPL-3.0-or-later，见原始 [LICENSE](LICENSE)；第三方许可与来源见 [docs/licenses.md](docs/licenses.md)。

Original author attribution, copyright, and license notices are retained as provided. Maintenance and new contributions in this fork are attributed to their respective contributors, not to the original author. The license is AGPL-3.0-or-later; see the original [LICENSE](LICENSE). Third-party licenses and sources are listed in [docs/licenses.md](docs/licenses.md).

当前整合源码纳入迁移上游 `main` 至审阅截止快照 `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb`（2026-10-11）的改进，并保留本 fork 的 Windows 工作及提交历史。早期 Windows 改动起于 `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`，先前发布提交为 `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`。2026-10-11 验证的安装程序对应旧提交 `59cb32d`，不包含本次上游整合。上游提交 [09d2a6b68132314df3555e87f97971a5ddce9088](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/commit/09d2a6b68132314df3555e87f97971a5ddce9088) 已为八个应用加入 release GUI 子系统设置，并以无控制台模式启动 LOK/PDF worker；这些不应描述为本 fork 的原创功能。

The integrated source includes migrated upstream `main` through the reviewed cutoff snapshot `60d82d0c5e96b47a7b2c7636f216ea61e9a5e1cb` (2026-10-11), while preserving this fork's Windows work and commit history. The earlier Windows changes began at `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`; the prior published commit is `59cb32d3fb0c55586a6e6763a57987ad38a6ff2d`. The installer verified on 2026-10-11 corresponds to the old `59cb32d` commit and does not include this upstream integration. Upstream commit [09d2a6b68132314df3555e87f97971a5ddce9088](https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/commit/09d2a6b68132314df3555e87f97971a5ddce9088) already adds release GUI subsystem settings to all eight apps and starts LOK/PDF workers without a console; those features are not claimed as original to this fork.

来源、贡献归属、补丁及构建限制见 [ORIGIN.md](ORIGIN.md)。本轮整合检查与未执行项目见 [docs/WINDOWS_INTEGRATION.md](docs/WINDOWS_INTEGRATION.md)。

See [ORIGIN.md](ORIGIN.md) for provenance, contribution attribution, patches, and build limits. This round's integration checks and items not run are in [docs/WINDOWS_INTEGRATION.md](docs/WINDOWS_INTEGRATION.md).

| App | Does | Engine |
| --- | --- | --- |
| ElyDoc | Word processing (DOCX, ODT) | LibreOfficeKit |
| ElyTable | Spreadsheets (XLSX, ODS, CSV) | LibreOfficeKit |
| ElyShow | Presentations (PPTX, ODP) | LibreOfficeKit, system media |
| ElyPDF | PDF reading and editing, OCR, signatures | MuPDF, Tesseract |
| ElyTree | Git client | Git |
| ElyMark | Markdown | Milkdown, CodeMirror in CEF |
| ElyMail, ElyAgent | Second release | — |

## Build (macOS)

```sh
scripts/doctor.sh                 # toolchain versions
scripts/fetch-references.sh       # upstream sources at pinned commits
scripts/build-libreoffice.sh      # patched LibreOffice 26.2.6.3 (about an hour)
scripts/build-mupdf.sh            # patched MuPDF 1.28.5
scripts/build-git.sh && scripts/fetch-git-lfs.sh
scripts/fetch-tessdata.sh && scripts/fetch-cef.sh
scripts/build-mark-content.sh     # ElyMark's page
cargo build -p ely-lok --bins -p ely-pdf-engine --bins
cargo run -p ely-doc              # or ely-table, ely-show, ely-pdf, ely-tree, ely-mark
```

Packages: `packaging/macos/release.sh` writes each app's `.app` and `.dmg`, the source archive and the upstream source archive (patched MuPDF, Git) to `dist/`. Linux recipes are in `packaging/linux` and have not been run here. Windows-specific source recipes and their staging requirements are described in [ORIGIN.md](ORIGIN.md); this candidate has not undergone a clean full build/install.

## Tests

`cargo test --workspace`, the window scripts in `tests/scripts` (macOS, Accessibility permission), and the benchmark `tests/perf/bench.py`.

## Documents

| | |
| --- | --- |
| `docs/help.md` | Help: opening and saving, recovery, offline limits, repositories |
| `docs/compatibility.md` | Versioned compatibility list |
| `docs/accessibility.md` | Accessibility and language review |
| `docs/perf/` | Benchmark records |
| `docs/licenses.md` | Licences and sources |

## Licence

AGPL-3.0-or-later (`LICENSE`). Third-party components keep their own licences; see `docs/licenses.md`.
