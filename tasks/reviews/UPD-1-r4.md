FAIL

crates/ely-ui/src/updates.rs:95 — [P2] 安装失败记录晚于应用重开，失败提示存在竞态。`ely_platform::install::finish` 在 `crates/ely-platform/src/install.rs:80` 先执行 `open`，返回后此处才写 `updates/failed.txt`；重开的应用只在启动时读取一次该文件（第 119–120 行）。真实 ElyTree 探针令 dmg 缺失，经透明包装调用真实 `/usr/bin/open` 并保持其参数和退出码，在启动请求之后暂停安装父进程；等重开窗口完成启动，再恢复安装进程。连续两次均出现：窗口启动时记录尚未写入，随后安装进程写入记录并退出，当前窗口始终没有失败提示，记录留待下次启动才读取。普通调度对照能生成失败提示，说明此次补建目录仍未保证提示交付。证据：`/tmp/upd1-r4-race-1.log`、`/tmp/upd1-r4-race-2.log`、`/tmp/upd1-r4-race-app.log`；复现步骤：`/tmp/upd1-r4-repro.log`。修复方向：失败记录完成写入及错误处理后再重开应用，并增加覆盖该先后顺序、断言本次重开即显示失败提示的回归测试。
