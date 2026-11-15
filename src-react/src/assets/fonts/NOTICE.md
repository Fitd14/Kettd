# 字体来源与授权

本目录只放一个字体文件，是**报头衬线**的子集：

| 文件 | 字数 | 体积 | 用途 |
|---|---|---|---|
| `kettd-masthead-serif.woff2` | 61 | 12.3 KB | 品牌字 `Kettd` 与六个视图报头（今天 / 收件箱 / 计划 / 回顾 / 知识库 / 设置） |

## 上游

- 字体：**Noto Serif SC**（思源宋体同源）
- 授权：SIL Open Font License 1.1，全文见同目录 [OFL-1.1.txt](OFL-1.1.txt)
- 源文件：本机系统字体 `C:\Windows\Fonts\NotoSerifSC-VF.ttf`（可变字重），裁切时固定 `wght=600`
- 处理：`subset-font`（harfbuzz 子集 + woff2 压缩），未改动字形轮廓

## 合规说明

- OFL 允许子集化与随软件分发；子集化属于「修改」，因此**保留许可证全文**并在本文件标明来源与改动方式。
- 子集版字体族名改为 `Kettd Serif`，避免与上游家族名混淆（OFL 的保留字体名条款）。
- 报头用字是固定文案，子集只覆盖这 61 个字符；界面里出现其它汉字时由 `--font-serif` 的后续字体承接，不会出现豆腐块。

## 重新生成

```powershell
npm i --no-save subset-font          # 只在生成时需要，不写入依赖
node scripts/fonts/subset-masthead.mjs
```

生成脚本入库在 `scripts/fonts/subset-masthead.mjs`，字符表也在脚本里（61 字）。
换源字体时用 `FONT_SRC` 指定 TTF 路径。产物应当是 12576 字节；差异过大说明字符表或字重轴被改动了。