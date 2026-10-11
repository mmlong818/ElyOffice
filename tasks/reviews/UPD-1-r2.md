FAIL

crates/ely-ui/src/updates.rs:195 — [P1] 安装发生在保存与退出确认之前，违反 PRD.md:113 的“更新前要求保存编辑内容”。实测在 ElyDoc 的 Untitled 1 输入文字，下载更新后按 Restart to update：先出现 `install: .../ElyDoc.app replaced`，随后才出现 `Save changes to Untitled 1?`；此时新包标记已在原路径，旧进程仍持有未保存内容。按 Esc 取消后，截图确认文档继续编辑，新包仍在原路径，更新状态停在 Installing，重启入口消失。证据：`/tmp/upd1-r2-save-only.log`、`/tmp/upd1-r2-evidence-3-ely-doc.log`。修复方向：将保存与退出确认作为替换包和注册重启的前置条件；取消时保留原包与可重试的下载完成状态，并覆盖未保存文档取消更新的回归测试。

crates/ely-ui/src/updates.rs:92 — [P1] 手动重查覆盖正在下载的 release，下载完成回调又直接修改当前 offer 的 phase（第 170–175 行），导致清单版本与待安装文件错配。真实 ElyTree 探针先查询 8.0.0 并开始下载 A.dmg，在 HTTP 服务暂停该下载期间手动重查得到 9.0.0/B.dmg，再放行 A：日志依次记录 `offers 9.0.0`、`update download: 8.0.0 saved .../A.dmg`；随后按 Restart to update，安装后的包标记为 A。用户对新 offer 的重启操作因此安装了此前请求的包。证据：`/tmp/upd1-r2-native.log`、`/tmp/upd1-r2-evidence-0-ely-tree.log`。修复方向：将检查、下载进度、完成结果和安装绑定同一 release 与操作身份，阻止重查覆盖进行中的更新，丢弃过期回调，并覆盖慢下载期间重查的回归测试。

crates/ely-ui/src/shell/root.rs:250 — [P2] 更新横幅被插在包含 TitleBar 的整个应用外壳之前，落入 macOS 原生窗口按钮所在区域，同时失去外壳提供的主题前景色。实测深色 ElyTree 手动检查得到 9.0.0 时，原生红黄绿按钮遮住更新文案开头；横幅其余文字为 RGB(0,0,0)，背景为 RGB(48,48,48)，截图实测对比度约 1.59:1，低于 PRD.md:750 的 4.5:1。ElyDoc 的就绪横幅也呈现同样遮挡。证据：`/tmp/upd1-r2-banner.log`。修复方向：将横幅放在主题化外壳的标题栏下方，使用主题前景色并保留原生窗口按钮区域；验证深浅主题下文案可读且无重叠。
