//! PDF 导出：把前端渲染好的自包含 HTML 交给离屏 WebView2 打印成 PDF。
//!
//! 为什么不用 Tauri 的 webview：主窗渲染的是应用界面，打印它只会得到 UI 截图；
//! 这里自己建一个不可见的窗口，加载正文 HTML，再调 `ICoreWebView2_7::PrintToPdf`。
//! 为什么不动 Tauri：tauri 1.8 依赖的 `webview2-com 0.19` 没有打印绑定，而 0.39 的
//! `ICoreWebView2_7` 自 2021 年运行时（1.0.1020.30）起就提供 PrintToPdf，单独引一份即可。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod inline;
mod webview;

/// 前端拼好的 HTML 上限。超过这个体量通常意味着把整篇图片都塞进了 data URI，
/// 继续往下送只会让打印超时，不如早点给出可读的错误。
pub(crate) const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;

/// WebView2 同一 user data 目录只允许一个环境实例，因此导出串行化。
static EXPORT_LOCK: Mutex<()> = Mutex::new(());

/// 打印实现。抽成参数是为了让失败清理与串行化能在单测里验证，不必起真的浏览器。
type Printer<'a> = dyn Fn(&str, &Path, &Path) -> Result<(), String> + 'a;

/// 把 HTML 打印成 PDF，写入 `target_path`，返回最终路径。
///
/// `user_data_dir` 必须是可写目录且**不能**放 `%TEMP%`：spike 里指向临时目录时
/// `CreateCoreWebView2Controller` 直接返回 `0x8000FFFF 灾难性故障`。
pub(crate) fn export_document(
    html: &str,
    assets_dir: &Path,
    user_data_dir: &Path,
    target_path: &str,
) -> Result<String, String> {
    run_export(html, assets_dir, user_data_dir, target_path, &webview::print_html)
}

fn run_export(
    html: &str,
    assets_dir: &Path,
    user_data_dir: &Path,
    target_path: &str,
    print: &Printer<'_>,
) -> Result<String, String> {
    validate_html(html)?;
    let target = crate::docx::resolve_target_path_with(target_path, "pdf")?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "目标文件夹不存在".to_string())?;
    }

    let _guard = EXPORT_LOCK
        .lock()
        .map_err(|_| "上一份 PDF 还没导出完，请稍后重试".to_string())?;

    // 先打到同目录的临时文件：打印失败时目标路径保持原样，不会留下半截 PDF。
    let temp_pdf = temp_pdf_path(&target);
    let _ = std::fs::remove_file(&temp_pdf);
    let html_with_images = inline::inline_assets(html, assets_dir);
    let printed = print(&html_with_images, user_data_dir, &temp_pdf);
    if let Err(err) = printed {
        let _ = std::fs::remove_file(&temp_pdf);
        return Err(err);
    }
    let bytes = match std::fs::read(&temp_pdf) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        Ok(_) => {
            let _ = std::fs::remove_file(&temp_pdf);
            return Err("打印结果是空文件，请重试".to_string());
        }
        Err(_) => {
            let _ = std::fs::remove_file(&temp_pdf);
            return Err("打印没有生成文件，请重试".to_string());
        }
    };
    let _ = std::fs::remove_file(&temp_pdf);
    crate::docx::write_bytes(&target, &bytes)?;
    Ok(target.display().to_string())
}

fn validate_html(html: &str) -> Result<(), String> {
    if html.trim().is_empty() {
        return Err("正文为空，没有可导出的内容".to_string());
    }
    if html.len() > MAX_HTML_BYTES {
        return Err("这篇文档太大，暂时无法导出 PDF".to_string());
    }
    Ok(())
}

fn temp_pdf_path(target: &Path) -> PathBuf {
    let stem = target
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "export".to_string());
    let name = format!(".{stem}.{}.tmp.pdf", std::process::id());
    match target.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = crate::docx::test_dir(&format!("pdf-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn empty_html_is_rejected() {
        let err = validate_html("   \n ").unwrap_err();
        assert!(err.contains("正文为空"), "{err}");
    }

    #[test]
    fn oversized_html_is_rejected() {
        let big = "x".repeat(MAX_HTML_BYTES + 1);
        let err = validate_html(&big).unwrap_err();
        assert!(err.contains("太大"), "{err}");
    }

    #[test]
    fn normal_html_passes() {
        assert!(validate_html("<!doctype html><p>你好</p>").is_ok());
    }

    #[test]
    fn temp_pdf_stays_next_to_target() {
        let target = PathBuf::from("C:/tmp/文档.pdf");
        let temp = temp_pdf_path(&target);
        assert_eq!(temp.parent(), target.parent());
        let name = temp.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with(".文档."), "{name}");
        assert!(name.ends_with(".tmp.pdf"), "{name}");
    }

    #[test]
    fn export_rejects_empty_html_before_touching_disk() {
        let err = export_document("", Path::new("."), Path::new("."), "C:/tmp/不存在的目录/x.pdf").unwrap_err();
        assert!(err.contains("正文为空"), "{err}");
    }

    #[test]
    fn failed_print_leaves_no_temp_file_and_keeps_target() {
        let dir = scratch("fail");
        let target = dir.join("文档.pdf");
        std::fs::write(&target, b"old").unwrap();
        let printer = |_html: &str, _data: &Path, temp: &Path| {
            std::fs::write(temp, b"half").unwrap();
            Err("打印超时，请稍后重试".to_string())
        };
        let err =
            run_export("<p>x</p>", &dir, &dir.join("ud"), &target.to_string_lossy(), &printer).unwrap_err();
        assert!(err.contains("打印超时"), "{err}");
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        assert!(!temp_pdf_path(&target).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_pdf_result_is_rejected() {
        let dir = scratch("empty");
        let target = dir.join("文档.pdf");
        let printer = |_html: &str, _data: &Path, temp: &Path| {
            std::fs::write(temp, b"").unwrap();
            Ok(())
        };
        let err =
            run_export("<p>x</p>", &dir, &dir.join("ud"), &target.to_string_lossy(), &printer).unwrap_err();
        assert!(err.contains("空文件"), "{err}");
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn successful_print_writes_target_and_cleans_temp() {
        let dir = scratch("ok");
        let target = dir.join("文档.pdf");
        let printer = |_html: &str, _data: &Path, temp: &Path| {
            std::fs::write(temp, b"%PDF-1.4 test").unwrap();
            Ok(())
        };
        let written =
            run_export("<p>x</p>", &dir, &dir.join("ud"), &target.to_string_lossy(), &printer).unwrap();
        assert_eq!(PathBuf::from(written), target);
        assert_eq!(std::fs::read(&target).unwrap(), b"%PDF-1.4 test");
        assert!(!temp_pdf_path(&target).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exports_are_serialized() {
        let dir = scratch("serial");
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for index in 0..2 {
            let dir = dir.clone();
            let active = active.clone();
            let peak = peak.clone();
            handles.push(std::thread::spawn(move || {
                let target = dir.join(format!("文档{index}.pdf"));
                let printer = move |_html: &str, _data: &Path, temp: &Path| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    active.fetch_sub(1, Ordering::SeqCst);
                    std::fs::write(temp, b"%PDF-1.4 test").unwrap();
                    Ok(())
                };
                run_export("<p>x</p>", &dir, &dir.join("ud"), &target.to_string_lossy(), &printer)
            }));
        }
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 真打印冒烟：要起真实的 WebView2，默认忽略。
    /// 跑法：`cargo test --offline pdf::smoke -- --ignored --nocapture`
    #[test]
    #[ignore = "需要真实 WebView2 运行时，手动跑"]
    fn smoke_prints_real_pdf() {
        // user data 放 target/ 而非 %TEMP%：沙箱里 %TEMP% 的 ACL 会让
        // CreateCoreWebView2Environment 直接返回 0x8000FFFF（灾难性故障）。
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("pdf-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("导出.pdf");
        let html = r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<style>@page { size: A4; margin: 20mm } body { font-family: "Microsoft YaHei", sans-serif }
table { border-collapse: collapse } td, th { border: 1px solid #ccc; padding: 4pt }</style>
</head><body><h1>导出链路验证</h1><p>中文段落与行内 <code>export_kb_pdf</code>。</p>
<table><thead><tr><th>列一</th><th>列二</th></tr></thead><tbody><tr><td>甲</td><td>乙</td></tr></tbody></table>
</body></html>"#;
        let printed = export_document(html, &dir, &dir.join("webview2"), &target.to_string_lossy())
            .expect("真实打印应成功");
        let bytes = std::fs::read(&printed).expect("读回 PDF");
        assert!(bytes.starts_with(b"%PDF-"), "不是 PDF：{:?}", &bytes[..4.min(bytes.len())]);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Type /Page"), "没有页面对象");
        println!("真实打印产物：{printed}（{} 字节）", bytes.len());
        let _ = std::fs::remove_dir_all(dir);
    }
}

