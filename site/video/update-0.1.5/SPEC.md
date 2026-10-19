# Ely Office 0.1.5 更新视频：规格

60.000 秒，1920×1080，60 fps，3600 帧。96 BPM，4/4：一拍 0.625 秒（37.5 帧），一小节 2.5 秒（150 帧），共 24 小节。段落切换都落在小节强拍上。

画面以**真实操作录屏**为主（已安装的 0.1.5，Windows，深色主题，中文界面）。录屏放在居中的“舞台”里，镜头在舞台内跟随指针平滑推近（像 Screen Studio）。段落之间用形变（shape morph）过渡：舞台收拢、变形成下一个应用的图标轮廓，再展开成下一个舞台。节奏从容，不快闪：任何画面至少停留一拍，形变用 0.6–1 秒。

## 时间表

| 小节 | 帧 | 段落 | 内容 |
| --- | --- | --- | --- |
| 1–2 | 0–300 | 开场 | 一个点在拍上呼吸，形变成六个应用图标的轮廓依次（每个至少一拍），最后化为“Ely Office 0.1.5”与“更顺手的一次更新”；第 3 小节强拍展开成第一个舞台 |
| 3–6 | 300–900 | 菜单 | ElyMark 右键 → 表格 ▸，指针斜穿“插入”行进入子菜单，子菜单不被抢走；安全三角叠在录屏上 |
| 7–10 | 900–1500 | 表格格内编辑 | ElyMark 点格即在格内编辑原文，Tab 到下一格，回车写回 |
| 11–16 | 1500–2400 | 右键照 Word / Excel / PowerPoint | ElyDoc（表格右键、插入行、拖边线）、ElyTable（表头右键）、ElyShow（缩略图滚轮、表格右键） |
| 17–21 | 2400–3150 | 还有 | 再开文件进同一窗口、清除最近文件、中文字形统一、macOS 已公证 |
| 22–24 | 3150–3600 | 结尾 | 舞台收拢成点，再展开成六个图标与“Ely Office 0.1.5”、平台与下载地址；第 24 小节末结束 |

音乐的能量：开场渐起（无鼓），第 3 小节进入律动，第 11 小节加厚，第 17 小节最高，第 22 小节强拍落定后渐弱到 60 秒收尾。每个段落切换前有半小节的上扬（riser），强拍上有柔和的冲击（impact）。

## 视觉

- 底色 `#1c1c1e` 到 `#252525` 的极轻径向渐变，细微颗粒；舞台外不放杂物。
- 强调色 `#3D7FE5`（ElyDoc 蓝）；各应用色：Doc `#3D7FE5`，Table `#2E9E5B`，Show `#F07B2A`，PDF `#E0344F`，Tree `#7E57C2`，Mark `#3E4A5C`（点缀 `#F2A900`）。
- 字体：中文 Noto Sans SC（`public/fonts/NotoSansSC.ttf`，可变字重），西文 Inter，等宽 JetBrains Mono。
- 舞台：`STAGE = { x: 120, y: 118, w: 1680, h: 945 }`，圆角 18，1px `rgba(255,255,255,0.08)` 描边，柔和的大投影。录屏 2048×1152，按舞台宽缩放（0.8203）。
- 段落标题在舞台上方左侧（y 30–100）：中文大字 + 一行小字说明。
- 动效：缓动用 `Easing.bezier(0.22, 1, 0.36, 1)`（出）与 `(0.65, 0, 0.35, 1)`（进出）；弹簧 `damping 200`；无闪烁，无高频抖动。

## 代码结构（Remotion）

```text
src/index.ts             registerRoot
src/Root.tsx             <Composition id="Update" …>
src/Video.tsx            各段 <Sequence>，音乐与音效
src/theme.ts             帧、节拍、颜色、STAGE、字体
src/design/              共用：Stage、Heading、Caption、KeyCap、Morph、背景、字体加载
src/scenes/Intro.tsx     开场
src/scenes/Outro.tsx     结尾
src/scenes/Menus.tsx …   录屏段落（由主导者写）
src/footage/             录屏的镜头跟随、事件读入
public/footage/          录屏与事件日志（不入库）
public/audio/            music.wav 与音效
```

`Stage` 负责舞台的外框、裁切与进出形变：`<Stage enter={{ from: 'ely-mark' }} exit={{ to: 'ely-doc' }}>{录屏}</Stage>`；进场时从图标轮廓（flubber 插值）展开成圆角矩形，出场时收拢成下一个图标的轮廓。裁切用 CSS `clip-path: path(...)`，录屏画在其中。

## 音频

- `public/audio/music.wav`：48 kHz 立体声，恰 60.000 秒，响度约 −16 LUFS，峰值 ≤ −1 dBTP，原创合成，无版权素材。
- `public/audio/sfx/`：`click.wav`（轻点击）、`menu.wav`（菜单弹出的轻“嗒”）、`key.wav`（键入）、`whoosh.wav`（形变）、`impact.wav`（强拍落定）、`tick.wav`（细小提示）。都短、干净、不刺耳。
