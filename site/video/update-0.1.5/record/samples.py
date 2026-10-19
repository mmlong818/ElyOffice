# Writes the sample files the recordings open. Usage: samples.py <dir>
import sys
from pathlib import Path

import docx
import openpyxl
from docx.oxml.ns import qn
from docx.shared import Pt
from openpyxl.styles import Font
from pptx import Presentation
from pptx.util import Inches, Pt as PPt

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)

ROWS = [
    ("模块", "负责人", "状态", "进度"),
    ("编辑器", "林夏", "已完成", "100%"),
    ("表格", "周沐", "进行中", "72%"),
    ("演示", "陈一", "进行中", "58%"),
    ("发布", "许言", "未开始", "0%"),
]

mark = """# 0.1.5 发布计划

这一版的重点是**更顺手**：菜单、表格与右键都照常用的软件来。

| 模块 | 负责人 | 状态 | 进度 |
| --- | --- | --- | --- |
| 编辑器 | 林夏 | **已完成** | 100% |
| 表格 | 周沐 | 进行中 | 72% |
| 演示 | 陈一 | 进行中 | 58% |
| 发布 | 许言 | 未开始 | 0% |

> [!TIP]
> 点任意一格即可在格内编辑，Tab 到下一格。

## 下一步

- 收集反馈 :rocket:
- 整理文档
"""
(out / "发布计划.md").write_text(mark, encoding="utf-8")
(out / "会议记录.md").write_text("# 会议记录\n\n- 确认发布时间\n", encoding="utf-8")
(out / "读书笔记.md").write_text("# 读书笔记\n\n摘录与想法。\n", encoding="utf-8")

d = docx.Document()
d.styles["Normal"].font.size = Pt(12)
# One installed face for every script: no missing-font banner.
FACE = "微软雅黑"
defaults = d.styles.element.find(qn("w:docDefaults"))
for fonts in [*defaults.iter(qn("w:rFonts")), *(style.element.get_or_add_rPr().get_or_add_rFonts()
                                               for style in d.styles if style.type == 1)]:
    for name in list(fonts.attrib):
        if name.endswith("Theme"):
            del fonts.attrib[name]
    for attr in ("w:ascii", "w:hAnsi", "w:eastAsia", "w:cs"):
        fonts.set(qn(attr), FACE)
d.add_heading("第三季度工作汇报", 1)
d.add_paragraph("本季度完成了编辑器与表格两大模块，演示模块进入收尾阶段。下表列出各模块的负责人与进度。")
t = d.add_table(rows=len(ROWS), cols=4)
t.style = "Table Grid"
for r, row in enumerate(ROWS):
    for c, value in enumerate(row):
        t.cell(r, c).text = value
d.add_paragraph("下季度计划：完善导出与打印，补齐快捷键。")
d.save(out / "工作汇报.docx")

wb = openpyxl.Workbook()
ws = wb.active
ws.title = "销售"
ws.append(["地区", "一月", "二月", "三月", "合计"])
data = [("华东", 128, 142, 160), ("华南", 96, 104, 121), ("华北", 88, 97, 110), ("西南", 64, 71, 83)]
for i, (name, *months) in enumerate(data, start=2):
    ws.append([name, *months, f"=SUM(B{i}:D{i})"])
ws.append(["合计", "=SUM(B2:B5)", "=SUM(C2:C5)", "=SUM(D2:D5)", "=SUM(E2:E5)"])
for cell in ws[1]:
    cell.font = Font(bold=True)
for col in "ABCDE":
    ws.column_dimensions[col].width = 12
wb.save(out / "季度销售.xlsx")

p = Presentation()
p.slide_width, p.slide_height = Inches(13.333), Inches(7.5)
s = p.slides.add_slide(p.slide_layouts[0])
s.shapes.title.text = "Ely Office 0.1.5"
s.placeholders[1].text = "更顺手的一次更新"
s = p.slides.add_slide(p.slide_layouts[5])
s.shapes.title.text = "模块进度"
table = s.shapes.add_table(len(ROWS), 4, Inches(1.2), Inches(2), Inches(10.9), Inches(3.6)).table
for r, row in enumerate(ROWS):
    for c, value in enumerate(row):
        table.cell(r, c).text = value
        table.cell(r, c).text_frame.paragraphs[0].runs[0].font.size = PPt(20)
for title in ("用户反馈", "编辑器", "表格", "演示", "文档", "快捷键", "右键菜单", "跨平台", "发布节奏", "下一步"):
    s = p.slides.add_slide(p.slide_layouts[1])
    s.shapes.title.text = title
    s.placeholders[1].text = "要点一\n要点二"
p.save(out / "季度汇报.pptx")
print("samples in", out)
