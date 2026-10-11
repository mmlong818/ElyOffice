# Ely Office

Offline-first desktop apps in Rust and GPUI. Files stay in your folders; no account, no cloud service.

## Source snapshot

This source-only candidate is based on upstream commit `ff92f26af0d96ba3a3742c67c3c4e31c05cc3890`. It is intentionally not synchronized with later upstream history. As of 2026-10-12, upstream `main` is `178625b6f3761c7a71ff6b1cd267643615e4fc48`, 202 commits ahead, and already includes its own release GUI/no-console implementation. This candidate does not claim that behavior is new or that this snapshot is current upstream. See [ORIGIN.md](ORIGIN.md) for provenance and scope.

The Windows build and installer scripts need externally prepared runtime stages and are not a clean-checkout, one-command full build. No clean full build/install was run for this candidate. Generated installers are unsigned; no binaries are included.

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
