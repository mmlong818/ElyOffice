FAIL

crates/ely-ui/src/updates.rs:59 — [P1] 启动检查绕过更新授权与默认设置，违反 PRD.md:113 的“自动检查更新默认关闭”及 QA-25。首次从安装好的 .app 启动、使用空白用户目录时，程序直接请求更新清单；本地端到端测试在任何用户操作之前已收到请求并记录 `offers 9.9.9`。当前设置模型也没有控制该请求的字段。修复方向：加入默认关闭的自动检查设置，启动检查以用户授权和离线设置为条件，并覆盖空白配置下零请求的测试。

crates/ely-ui/src/updates.rs:67 — [P2] 更新检查仅由启动路径调用一次，缺少 PRD.md:113 要求的手动检查入口。具体场景：启动时断网，唯一一次检查失败后，恢复网络并继续使用应用，用户无法重新检查、获得更新横幅或下载更新；`check` 为私有方法，调用点只有第 59 行，菜单与设置均未接入检查动作。修复方向：提供可重复执行的手动检查动作，接入应用界面，并测试首次失败后联网重查。

crates/ely-platform/src/install.rs:65 — [P1] 新包替换成功后，删除旧包的错误被当作整个安装失败返回，更新状态与磁盘事实分离，失败路径也未恢复原版本，违反 PRD §4.2、QA-26。实测在旧包 `Contents/version` 上设置 `chflags uchg` 后调用真实 dmg 安装，结果为 `install=Err(PermissionDenied: Operation not permitted)`、`live=Ok("new")`、`backup=Ok("old")`。调用方因此跳过 `reopen_after_exit` 和正常退出，显示更新失败，原路径却已换成新版；重试还会卡在删除同一旧包。修复方向：明确安装提交点，将提交后的备份清理错误与安装结果分开处理，保证提交前失败恢复旧包、提交后结果与实际版本一致，并加入清理失败的回归用例。

tests/scripts/update.sh:54 — [P2] 重启断言只按命令行中的 .app 路径执行 `pgrep -f`，重启辅助进程也满足条件，测试会把启动失败判为成功。实测仅在临时脚本中通过 PATH 将 `open` 换成 `sleep 30; exit 1`，完整测试仍输出 `ok: ElyTree downloads, installs and reopens its update`；通过断言的唯一进程是 `/bin/sh .../bin/open .../Applications/ElyTree.app`，新版 ElyTree 实际未启动。修复方向：断言新版应用自身的可执行文件身份、不同于旧进程的 PID，以及窗口或启动完成信号；让该 `open` 失败变异稳定触发测试失败。
