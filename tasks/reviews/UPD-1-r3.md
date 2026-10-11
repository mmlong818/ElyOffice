FAIL

crates/ely-ui/src/updates.rs:224 — [P1] 取消更新退出后，`pending` 仍保留，下一次普通退出继续安装并重开应用，违反“取消即什么也不装”的修复契约。真实 ElyMark 探针输入未保存文字，下载完成后点击 Restart to update，再点击 Cancel；随后仅从应用菜单选择 Quit ElyMark，并对未保存内容选择 Don't Save，更新标记仍被装入原包，新的应用进程再次启动。日志记录普通退出后的 `update: installs after this quit`；当前回归测试取消后再次点击 Restart，遗漏了这条路径。证据：`/tmp/upd1-r3-cancel.log`、`/tmp/upd1-r3-cancel-app.log`。修复方向：将待安装状态绑定这次更新退出请求，在取消或退出流程中止时清除；保留已下载文件供再次明确点击 Restart，并覆盖取消后普通退出的测试。

crates/ely-ui/src/updates.rs:131 — [P2] 下载中或已下载时手动检查直接返回，使已关闭的更新横幅无法恢复，下载完成后的安装入口随之丢失。真实 ElyTree 探针下载 8.0.0 完成，关闭横幅的 x，再选择 Check for Updates…；检查函数只记录 `a release is already on its way`，`dismissed` 持续为 true，AX 树中 Restart to update 消失，用户在本次运行中无法重新进入安装操作。下载期间关闭横幅同样进入此状态。证据：`/tmp/upd1-r3-dismiss.log`、`/tmp/upd1-r3-tree-dark.log`。修复方向：用户手动检查时重新显示现有下载状态，同时保持该 release 和下载文件的绑定；覆盖下载中及下载完成后关闭横幅再手动检查的回归测试。

crates/ely-ui/src/updates.rs:84 — [P2] 安装失败记录的写入错误被 `.ok()` 丢弃，而安装子进程的 stderr 在第 236 行被重定向到空设备，导致失败静默。以下载目录被清理的状态运行真实安装入口：dmg 与 `updates` 目录均已消失，挂载失败后原应用正常重开，写入 `updates/failed.txt` 因父目录缺失失败；重开窗口没有失败提示，记录文件也不存在。保留记录目录的对照探针能够显示同一安装错误。证据：`/tmp/upd1-r3-missing-parent.log`、`/tmp/upd1-r3-missing-parent-reopened.log`、`/tmp/upd1-r3-failure.log`。修复方向：确保故障记录目录存在，处理记录写入错误，并保证持久化失败时重开的应用仍能展示安装失败；覆盖下载目录被清理的安装失败测试。
