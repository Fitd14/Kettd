# -*- coding: utf-8 -*-
"""生成 kb 编辑器视觉情绪板（board 技能 Phase 5，ImageGen 降级为渐变格）"""
import os

schemes = [
  {
    "id":"a","name":"宣纸墨","desc":"宣纸米白为底、浓墨为字、黛青点睛。最素净的一套：把纸留给字，沉浸感来自极致克制。",
    "keywords":["素净","留白","墨韵","克制"],
    "font":"Noto Serif SC",
    "primary":["#46557A","#93A5C0","#2E3A52","#EEF1F6","#1D2433"],
    "secondary":["#B08356","#D4BC9A","#8A6238","#F3EBDD","#5C4326"],
    "btn_bg":"#46557A","btn_color":"#fff","input_border":"#93A5C0",
    "card_border":"#E3E7EF","tag_bg":"#EEF1F6","tag_color":"#2E3A52","toggle_bg":"#46557A",
    "grads":[("#F7F5EE","#E9E4D6","宣纸底"),("#EEF1F6","#C9D2E2","黛青留白"),("#2E3A52","#46557A","浓墨字"),("#F3EBDD","#E0D2B8","赭石批注")]
  },
  {
    "id":"b","name":"青瓷","desc":"青瓷绿是传统釉色里最「清新」的一支：米白偏青的底、青瓷主色、暖米辅色，像雨过天青的釉面。",
    "keywords":["雨过天青","温润","清新","釉面"],
    "font":"LXGW WenKai",
    "primary":["#6FA08B","#A8C7B8","#4E7A67","#EAF2EE","#33514A"],
    "secondary":["#C8A580","#E0CDB2","#9E7D58","#F7F1E6","#6B543A"],
    "btn_bg":"#5F8D7B","btn_color":"#fff","input_border":"#A8C7B8",
    "card_border":"#DCE8E1","tag_bg":"#EAF2EE","tag_color":"#4E7A67","toggle_bg":"#6FA08B",
    "grads":[("#F5F8F3","#E8F0E9","瓷底"),("#EAF2EE","#C9DFD4","青瓷晕"),("#4E7A67","#6FA08B","釉色"),("#F7F1E6","#EBDFC9","暖米辅")]
  },
  {
    "id":"c","name":"竹月","desc":"竹青配月白：绿意来自竹、清冷来自月白蓝，一笔朱砂作印。三色关系像一幅小写意。",
    "keywords":["竹青","月白","写意","透气"],
    "font":"Noto Sans SC",
    "primary":["#7C9A6D","#A9BFA0","#5A7A4C","#EFF3EA","#3C5233"],
    "secondary":["#7FA8C9","#B3CEDF","#5A85A6","#EAF2F7","#39586E"],
    "btn_bg":"#5A7A4C","btn_color":"#fff","input_border":"#A9BFA0",
    "card_border":"#E2EADB","tag_bg":"#EFF3EA","tag_color":"#5A7A4C","toggle_bg":"#7C9A6D",
    "grads":[("#F8F9F2","#EDEFDF","竹月底"),("#EFF3EA","#D6E2CE","竹青"),("#5A85A6","#7FA8C9","月白"),("#F9EDEA","#EFD6D0","朱砂印")]
  },
  {
    "id":"d","name":"朱砂宣","desc":"暖宣纸打底、朱砂作主色：像一枚盖在宣纸上的印。沉浸感最浓、识别度最高的一套。",
    "keywords":["朱砂","暖宣","印章","沉浸"],
    "font":"Noto Serif SC",
    "primary":["#C0574F","#E0A29C","#96423C","#F9EDEA","#6E2B26"],
    "secondary":["#4A4A52","#A3A3AC","#33333A","#EDEDF0","#1F1F24"],
    "btn_bg":"#C0574F","btn_color":"#fff","input_border":"#E0A29C",
    "card_border":"#F2E2DE","tag_bg":"#F9EDEA","tag_color":"#96423C","toggle_bg":"#C0574F",
    "grads":[("#FAF6EE","#F1E9DA","暖宣底"),("#F9EDEA","#EFD0CA","朱砂晕"),("#96423C","#C0574F","印色"),("#EDEDF0","#D9D9E0","墨灰")]
  },
]

project_title = "Kettd · kb 知识库编辑器"
persona_line = "Persona: 小柯 · 单人 Windows 桌面 · 记下来就安全"
jtbd_line = "JTBD: 写知识时像在宣纸上落笔，工具隐入纸背"

BTN_PRIMARY = "保存知识"
BTN_SECONDARY = "预览"
INPUT_PLACEHOLDER = "搜索知识库…"
CARD_TITLE = "Rust 入门笔记"
CARD_DESC = "上周整理的安装与环境配置"
TAG_A = "#rust"
TAG_B = "#随笔"

LOCKED_CSS = """* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: "PingFang SC", -apple-system, sans-serif;
  background: #FFFFFF;
  color: #1F2937;
  padding: 48px 40px;
  min-height: 100vh;
}
.container { max-width: 100%; margin: 0 auto; }
.scheme-nav {
  display: flex;
  gap: 0;
  margin-bottom: 48px;
  border-bottom: 1px solid #D1D5DB;
}
.scheme-tab {
  padding: 10px 24px;
  font-size: 13px;
  font-weight: 400;
  color: #6B7280;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}
.scheme-tab:hover { color: #1F2937; }
.scheme-tab.active {
  color: #1F2937;
  font-weight: 600;
  border-bottom-color: #1F2937;
}
.scheme-panel { display: none; }
.scheme-panel.active { display: block; }
.mood-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
  margin-bottom: 40px;
}
.mood-img {
  height: 100px;
  display: flex;
  align-items: flex-end;
  padding: 8px;
}
.mood-img span {
  font-size: 9px;
  color: #fff;
  background: rgba(0,0,0,0.4);
  padding: 2px 6px;
}
.row-type {
  display: flex;
  align-items: center;
}
.type-display {
  flex: 0 0 50%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
.type-display .aa {
  font-size: 120px;
  font-weight: 600;
  line-height: 1;
  letter-spacing: -2px;
}
.type-display .font-name {
  font-size: 11px;
  color: #6B7280;
  margin-top: 10px;
}
.intro {
  flex: 1;
  padding-left: 40px;
}
.intro h2 {
  font-size: 15px;
  font-weight: 600;
  color: #1F2937;
  margin-bottom: 10px;
}
.intro p {
  font-size: 13px;
  color: #6B7280;
  line-height: 1.7;
  margin-bottom: 2px;
}
.intro .keywords {
  display: flex;
  gap: 6px;
  margin-top: 12px;
}
.intro .kw {
  font-size: 10px;
  padding: 2px 8px;
  border: 1px solid #D1D5DB;
  color: #4B5563;
}
.divider {
  height: 1px;
  background: #D1D5DB;
  margin: 40px 0;
}
.row-colors {
  display: flex;
  gap: 40px;
}
.row-colors > div {
  flex: 1;
  min-width: 25%;
}
.color-group {
  display: flex;
  gap: 0;
  height: 160px;
  width: 100%;
}
.color-strip {
  height: 100%;
}
.color-module-title {
  font-size: 11px;
  font-weight: 600;
  color: #6B7280;
  margin-bottom: 10px;
  text-transform: uppercase;
  letter-spacing: 1px;
}
.row-comps {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0;
}
.comp-col {
  padding: 0 15px;
  border-right: 1px solid #D1D5DB;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.comp-col:first-child { padding-left: 0; }
.comp-col:last-child { border-right: none; padding-right: 0; }
.comp-col-title {
  font-size: 10px;
  color: #6B7280;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  margin-bottom: 4px;
}
.btn {
  display: inline-block;
  padding: 6px 16px;
  font-size: 11px;
  font-weight: 600;
  border: none;
  cursor: pointer;
  text-align: center;
  white-space: nowrap;
}
.input-demo {
  padding: 6px 10px;
  border: 1px solid #D1D5DB;
  font-size: 11px;
  background: #F9FAFB;
  width: 120px;
  color: #6B7280;
}
.card-demo {
  border: 1px solid #D1D5DB;
  padding: 10px;
  max-width: 140px;
}
.card-demo .card-title {
  font-size: 11px;
  font-weight: 600;
  margin-bottom: 3px;
}
.card-demo .card-desc {
  font-size: 10px;
  color: #6B7280;
}
.tag {
  display: inline-block;
  padding: 2px 6px;
  font-size: 9px;
  font-weight: 500;
}
.toggle {
  width: 30px; height: 16px;
  border-radius: 8px;
  position: relative;
}
.toggle::after {
  content: '';
  position: absolute;
  width: 12px; height: 12px;
  background: #fff;
  border-radius: 50%;
  top: 2px; right: 2px;
}
.header {
  margin-bottom: 32px;
}
.header h1 {
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 6px;
}
.header p {
  font-size: 12px;
  color: #6B7280;
}"""


def render_scheme(s, active=False):
    active_cls = " active" if active else ""
    # ImageGen 不可用降级：4 格全部 CSS linear-gradient（技能降级条款）
    mood_html = ""
    for c1, c2, label in s["grads"]:
        mood_html += (
            '<div class="mood-img" style="background:linear-gradient(135deg, '
            + c1 + ', ' + c2 + ');"><span>' + label + '</span></div>\n'
        )
    mood_html += (
        '<div class="mood-img" style="background:linear-gradient(135deg, '
        + s["primary"][0] + ', ' + s["secondary"][0] + ');"><span>色彩延伸</span></div>\n'
    )
    flexes = [8, 4, 3, 2, 2]
    pri = "".join('<div class="color-strip" style="flex:%d;background:%s;"></div>' % (fx, c) for c, fx in zip(s["primary"], flexes))
    sec = "".join('<div class="color-strip" style="flex:%d;background:%s;"></div>' % (fx, c) for c, fx in zip(s["secondary"], flexes))
    kw = "".join('<span class="kw">%s</span>' % k for k in s["keywords"])

    return ('<div class="scheme-panel' + active_cls + '" id="panel-' + s["id"] + '">\n'
      '  <div class="mood-row">' + mood_html + '</div>\n'
      '  <div class="row-type">\n'
      '    <div class="type-display">\n'
      '      <div class="aa" style="font-family:\'' + s["font"] + '\',serif;">Aa</div>\n'
      '      <div class="font-name">' + s["font"] + '</div>\n'
      '    </div>\n'
      '    <div class="intro">\n'
      '      <h2>' + s["name"] + '</h2>\n'
      '      <p>' + s["desc"] + '</p>\n'
      '      <div class="keywords">' + kw + '</div>\n'
      '    </div>\n'
      '  </div>\n'
      '  <div class="divider"></div>\n'
      '  <div class="row-colors">\n'
      '    <div><div class="color-module-title">Primary</div><div class="color-group">' + pri + '</div></div>\n'
      '    <div><div class="color-module-title">Secondary</div><div class="color-group">' + sec + '</div></div>\n'
      '  </div>\n'
      '  <div class="divider"></div>\n'
      '  <div class="row-comps">\n'
      '    <div class="comp-col">\n'
      '      <div class="comp-col-title">Buttons</div>\n'
      '      <button class="btn" style="background:' + s["btn_bg"] + ';color:' + s["btn_color"] + ';">' + BTN_PRIMARY + '</button>\n'
      '      <button class="btn" style="background:transparent;color:' + s["btn_bg"] + ';border:1px solid ' + s["btn_bg"] + ';">' + BTN_SECONDARY + '</button>\n'
      '    </div>\n'
      '    <div class="comp-col">\n'
      '      <div class="comp-col-title">Input</div>\n'
      '      <input class="input-demo" style="border-color:' + s["input_border"] + ';" value="' + INPUT_PLACEHOLDER + '" readonly>\n'
      '    </div>\n'
      '    <div class="comp-col">\n'
      '      <div class="comp-col-title">Card</div>\n'
      '      <div class="card-demo" style="border-color:' + s["card_border"] + ';">\n'
      '        <div class="card-title">' + CARD_TITLE + '</div>\n'
      '        <div class="card-desc">' + CARD_DESC + '</div>\n'
      '      </div>\n'
      '    </div>\n'
      '    <div class="comp-col">\n'
      '      <div class="comp-col-title">Tags</div>\n'
      '      <span class="tag" style="background:' + s["tag_bg"] + ';color:' + s["tag_color"] + ';">' + TAG_A + '</span>\n'
      '      <span class="tag" style="background:' + s["tag_bg"] + ';color:' + s["tag_color"] + ';">' + TAG_B + '</span>\n'
      '      <div class="toggle" style="background:' + s["toggle_bg"] + ';"></div>\n'
      '    </div>\n'
      '  </div>\n'
      '</div>')


tabs_html = ""
for i, s in enumerate(schemes):
    ac = " active" if i == 0 else ""
    tabs_html += '<div class="scheme-tab' + ac + '" onclick="switchTab(\'' + s["id"] + '\')">' + s["name"] + '</div>'

panels_html = ""
for i, s in enumerate(schemes):
    panels_html += render_scheme(s, active=(i == 0))

html = ('<!DOCTYPE html>\n<html lang="zh-CN">\n<head>\n<meta charset="UTF-8">\n'
  '<meta name="viewport" content="width=device-width, initial-scale=1.0">\n'
  '<title>' + project_title + ' — Design System Mood Board</title>\n<style>\n'
  + LOCKED_CSS +
  '\n</style>\n</head>\n<body>\n<div class="container">\n'
  '  <div class="header">\n'
  '    <h1>' + project_title + ' &mdash; Design System Mood Board</h1>\n'
  '    <p>' + persona_line + ' &middot; ' + jtbd_line + '</p>\n'
  '  </div>\n'
  '  <div class="scheme-nav">' + tabs_html + '</div>\n'
  '  ' + panels_html + '\n'
  '</div>\n<script>\n'
  'function switchTab(id) {\n'
  "  document.querySelectorAll('.scheme-tab').forEach(function(t) { t.classList.remove('active'); });\n"
  "  document.querySelectorAll('.scheme-panel').forEach(function(p) { p.classList.remove('active'); });\n"
  "  event.currentTarget.classList.add('active');\n"
  "  document.getElementById('panel-' + id).classList.add('active');\n"
  '}\n'
  '</script>\n</body>\n</html>')

os.makedirs("spark-output/board", exist_ok=True)
board_path = "spark-output/board/kb-editor-moodboard.html"
with open(board_path, "w", encoding="utf-8") as f:
    f.write(html)
print("Done: %.0f KB" % (os.path.getsize(board_path) / 1024))
