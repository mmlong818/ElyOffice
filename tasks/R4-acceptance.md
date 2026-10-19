# R4 验收核对表（ElyMark 对照 Typora，2026-10-14）

标准：每项功能可用，不求与 Typora 一一相同（用户 2026-10-14 定）。`[x]` 审查通过；`[~]` 三轮审查用尽，最后一轮的问题已修、各有回归测试，未再审。

## 任务

| 条目 | 状态 | 核对 | 证据 | 遗留 |
| --- | --- | --- | --- | --- |
| R4.0 前置 | [x] | 审查改用 gpt-6-astra max fast；PRD §14 与 `agents.md` 记下决定 | `tasks/reviews/R4.0-r1.md` | — |
| R4.1 菜单图标行 | [~] | 右键菜单的剪贴板与格式图标行可点、可键盘、禁用时灰显 | 组件测试；真窗口截图 `menu-right-text.png` | — |
| R4.2 行内扩展 | [~] | 表情、行内 HTML、脚注上标、删除线在可视里画出 | 组件测试；真窗口截图 `visual.png` | — |
| R4.3 块扩展 | [~] | 提示块、`[toc]`、front matter 框、嵌套列表标记、分隔线画出 | 同上 | — |
| R4.4 导出与复制一致 | [~] | 导出 HTML、纯文本、富文本与可视同一读法 | ElyMark 测试 | — |
| R4.5 按下与右键定位 | [~] | 按下块插入点落在所按处；右键事件带目标 | 组件测试；真窗口右键复制链接地址 | — |
| R4.6 行内格式命令 | [~] | Ctrl+B、Ctrl+Shift+`、Alt+Shift+5 包住所选，一步撤销 | 真窗口 `keys.txt`、`shifted.txt` | 128 KiB 内块极多时重读可占界面线程逾 100 毫秒；超过 128 KiB 的文件可视打开落在第 2 行 |
| R4.7 块命令 | [~] | Ctrl+1/0、Ctrl+Shift+[ ]、菜单“段落 › 标题 1” | 真窗口 `keys.txt`、`menus.txt`、`shifted.txt` | 列表与引用的切换不做写后重读核对 |
| R4.8 表格移动与整理 | [~] | Alt+↑ 移行、Alt+→ 移列，撤销两步还原 | 真窗口 `table.txt` | 编辑器本身仍按 LF 分行，文档层把 CR 与 LF 之间的位置挪到行尾 |
| R4.9 菜单与快捷键 | [x] | 段落、格式、视图菜单与键；F8、F9、缩放、Ctrl+/、Ctrl+Shift+C | 真窗口 `keys.txt`、截图 `menu-paragraph.png` | 见下“R4.11 查出” |
| R4.10 右键菜单 | [~] | 按目标变化；复制链接地址；中文菜单 | 真窗口 `menus.txt`、截图 `menu-right-*.png`、`zh-right.png` | 与内容完全相同的后来复制分不出，旧转换照样落下 |
| R4.12 单实例交文件 | [x] | 第二份交出文件即退出，第一份开新标签并到前台 | 真窗口 `.work/R4.12` | macOS 的 `one-instance.sh` 未在 Mac 上跑 |
| R4.13 中文显示 | [x] | 同一词一种字体；查找框、排序筛选、提示中文 | 真窗口 `zh-*.png` | LibreOffice 自己的文字仍为英文（界面语言固定英文） |
| R4.11 验收 | [~] | 真窗口逐项操作；帮助、PRD、`agents.md` 随之 | 本表；`.work/R4.11` | 三轮都只查出测试脚本的备份还原问题（不涉产品代码），均已修并有回归（`backup-test.ps1`）；第 3 轮的修复未再审 |

## R4.11 查出并已修

- 带 Shift 的标点键在真键盘上按不出：三端的 gpui 都把 Shift 加该键报成字符并去掉 Shift，R4.9 在 Windows 写的是键名。Ctrl+Shift+[ ] ` = -、Alt+Shift+5 改绑所出的字符；真窗口逐个按过（`shifted.txt`、`keys.txt`），Windows 键表测试在旧绑定上失败（`.work/R4.11/red-keys.log`）。
- Ctrl+Shift+0（实际大小）在 Windows 上收不到：中文输入法把它登记为热键（`HKCU\Control Panel\Input Method\Hot Keys\00000104`）。保留它，Windows 另绑 Ctrl+Shift+9，菜单显示后者；真窗口按过。

## 真窗口（Windows，2026-10-14）

脚本在 `.work/R4.11`（`keys.ps1`、`menus.ps1`、`shifted.ps1`、`table.ps1`、`zh.ps1`），以真键盘与鼠标事件驱动调试构建，保存后读文件字节、读 `look.json` 与剪贴板核对；每次运行前后备份并还原 ElyMark 的会话、最近、视图与设置文件。结果 21 项全部通过（`shifted.txt` 里一次 Alt+↑ 失败是脚本在未保存的标签上改写文件，ElyMark 正确拒绝重载；改用新开的文件后通过）。

未在真窗口核对：macOS 与 Linux；导出与打印的系统对话框（单测覆盖）。

## 执行纪律

| 要求 | 核对 |
| --- | --- |
| 任务清单落盘，按序完成 | `tasks/TASKS.md` R4.0–R4.13，自上而下；R4.12、R4.13 为用户插入的急件，先于 R4.10、R4.11 |
| 不用 worktree、分支、子 agent、并行 | 全程在 main 上一条一条做 |
| 每条 codex 审查（gpt-6-astra，max，fast），最多三轮 | `tasks/reviews/R4.*-r*.md` |
| 问题先修再复审，修复带回归测试 | 每轮的红绿日志在 `.work/R4.*-fix*` |
| 文件不超过 500 行，注释一行十字以内 | `tests/scripts/comments.py` 通过；R4 改动的文件都在 500 行内 |
| 推送 | 组件库各提交已推到 Gitea；ElyOffice 本地领先 origin 5 个提交，待用户允许推送 |
