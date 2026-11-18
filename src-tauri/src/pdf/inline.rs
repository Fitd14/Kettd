//! 把 HTML 里的 `assets/<sha>.<ext>` 图片内联成 data URI。
//!
//! 为什么在 Rust 做：`NavigateToString` 加载的文档没有可用的资源根，
//! `assets/` 相对路径解析不到；而图片字节本来就在数据目录里，读一遍最省事。
//! 读不到或超过上限时，整张图替换成 `[图片缺失：alt]` 文本，避免打印出破图。

use std::path::Path;

/// 单张图上限与 docx 导出保持一致（5 MB），避免把 PDF 撑爆。
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

pub(crate) fn inline_assets(html: &str, assets_dir: &Path) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let Some(start) = find_img_tag(rest) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let tag_body = &rest[start..];
        let Some(end) = tag_body.find('>') else {
            out.push_str(tag_body);
            break;
        };
        let tag = &tag_body[..=end];
        match src_value(tag) {
            Some(src) if src.starts_with("assets/") => {
                let alt = attr_value(tag, "alt").unwrap_or_default();
                match data_uri(assets_dir, src) {
                    Some(uri) => out.push_str(&replace_src(tag, &uri)),
                    None => out.push_str(&format!("[图片缺失：{}]", if alt.is_empty() { "未命名" } else { &alt })),
                }
            }
            _ => out.push_str(tag),
        }
        rest = &tag_body[end + 1..];
    }
    out
}

fn find_img_tag(text: &str) -> Option<usize> {
    let mut index = 0;
    while let Some(pos) = text[index..].find('<') {
        let at = index + pos;
        let tail = text[at..].as_bytes();
        // 逐字节比较，避免按字节切片切开多字节字符。
        let is_img = tail.len() >= 4
            && tail[1].eq_ignore_ascii_case(&b'i')
            && tail[2].eq_ignore_ascii_case(&b'm')
            && tail[3].eq_ignore_ascii_case(&b'g');
        if is_img {
            let after = tail.get(4).copied();
            if matches!(
                after,
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/') | Some(b'>')
            ) {
                return Some(at);
            }
        }
        index = at + 1;
        if index >= text.len() {
            break;
        }
    }
    None
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=\"");
    let start = lower.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn src_value(tag: &str) -> Option<&str> {
    attr_value(tag, "src")
}

fn replace_src(tag: &str, uri: &str) -> String {
    let lower = tag.to_ascii_lowercase();
    let Some(start) = lower.find("src=\"") else {
        return tag.to_string();
    };
    let value_start = start + 5;
    let Some(len) = tag[value_start..].find('"') else {
        return tag.to_string();
    };
    let value_end = value_start + len;
    let mut out = String::with_capacity(tag.len() + uri.len());
    out.push_str(&tag[..value_start]);
    out.push_str(uri);
    out.push_str(&tag[value_end..]);
    out
}

fn data_uri(assets_dir: &Path, src: &str) -> Option<String> {
    let name = src.strip_prefix("assets/")?;
    // 只接受单层文件名：挡住 `../` 与 Windows 分隔符。
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains(':')
    {
        return None;
    }
    let path = assets_dir.join(name);
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() as usize > MAX_IMAGE_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mime = match path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()) {
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        Some(ext) if ext == "gif" => "image/gif",
        Some(ext) if ext == "webp" => "image/webp",
        Some(ext) if ext == "bmp" => "image/bmp",
        Some(ext) if ext == "svg" => "image/svg+xml",
        _ => return None,
    };
    Some(format!("data:{mime};base64,{}", crate::infra::text_codec::encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = crate::docx::test_dir(&format!("inline-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn inlines_asset_image() {
        let dir = scratch("ok");
        std::fs::write(dir.join("abc.png"), PNG_1X1).unwrap();
        let html = r#"<p>x</p><img src="assets/abc.png" alt="图" width="10">"#;
        let out = inline_assets(html, &dir);
        assert!(out.contains("data:image/png;base64,"), "{out}");
        assert!(out.contains("alt=\"图\""), "{out}");
        assert!(out.contains("width=\"10\""), "{out}");
        assert!(!out.contains("assets/abc.png"), "{out}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_image_becomes_text() {
        let dir = scratch("missing");
        let html = r#"<img src="assets/none.png" alt="配图">"#;
        let out = inline_assets(html, &dir);
        assert_eq!(out, "[图片缺失：配图]");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = scratch("traversal");
        let html = r#"<img src="assets/../secret.png" alt="">"#;
        let out = inline_assets(html, &dir);
        assert!(out.starts_with("[图片缺失"), "{out}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn leaves_other_images_and_text_untouched() {
        let dir = scratch("other");
        let html = r#"<p>前</p><img src="https://example.com/a.png"><p>后</p>"#;
        assert_eq!(inline_assets(html, &dir), html);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn handles_multiple_images() {
        let dir = scratch("multi");
        std::fs::write(dir.join("a.png"), PNG_1X1).unwrap();
        let html = r#"<img src="assets/a.png"><img src="assets/missing.png" alt="缺">"#;
        let out = inline_assets(html, &dir);
        assert!(out.contains("data:image/png;base64,"), "{out}");
        assert!(out.contains("[图片缺失：缺]"), "{out}");
        let _ = std::fs::remove_dir_all(dir);
    }
}