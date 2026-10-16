// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-format document parser and ASR context terminology extractor.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 判定是否属于支持的多模态图片
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "svg"
            )
        })
        .unwrap_or(false)
}

/// 判定是否属于可解析的参考文档
pub fn is_document_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "txt"
                    | "md"
                    | "markdown"
                    | "json"
                    | "csv"
                    | "log"
                    | "yaml"
                    | "yml"
                    | "xml"
                    | "html"
                    | "sql"
                    | "rs"
                    | "ts"
                    | "js"
                    | "py"
                    | "pdf"
                    | "docx"
                    | "doc"
            )
        })
        .unwrap_or(false)
}

/// 从常见文档格式中抽取关键正文内容（支持 txt / md / json / csv / docx / pdf）
pub fn extract_document_text(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let text = match ext.as_str() {
        "txt" | "md" | "markdown" | "json" | "csv" | "log" | "yaml" | "yml" | "xml" | "html"
        | "sql" | "rs" | "ts" | "js" | "py" => extract_plain_text(path),
        "docx" => extract_docx_text(path).or_else(|| extract_docx_via_cli(path)),
        "pdf" => extract_pdf_text(path),
        _ => None,
    };

    text.map(|t| {
        let trimmed = t.trim();
        // 限制单篇文档注入内容在 30,000 字符内，避免大模型上下文溢出
        if trimmed.chars().count() > 30000 {
            let truncated: String = trimmed.chars().take(30000).collect();
            format!("{}\n\n[...该文档内容较长，已自动截取前 30,000 字...]", truncated)
        } else {
            trimmed.to_string()
        }
    })
}

/// 读取纯文本文件
fn extract_plain_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| {
        String::from_utf8(bytes.clone())
            .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).to_string())
    })
}

/// 纯 Rust 原生解析 DOCX (解压读取 word/document.xml 中的段落与 <w:t> 节点)
fn extract_docx_text(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut xml_file = archive.by_name("word/document.xml").ok()?;
    let mut xml_content = String::new();
    xml_file.read_to_string(&mut xml_content).ok()?;

    parse_docx_xml(&xml_content)
}

/// 使用 quick-xml 快速提取 Word document.xml 文本
fn parse_docx_xml(xml: &str) -> Option<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut result = String::new();
    let mut in_text = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                "w:t" => in_text = true,
                "w:p" => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == "w:t" {
                    in_text = false;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    let s = &e[..];
                    if let Ok(txt) = quick_xml::escape::unescape(s) {
                        result.push_str(&txt);
                    } else {
                        result.push_str(s);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if result.trim().is_empty() {
        None
    } else {
        Some(result)
    }
}

/// 备用方式：通过系统 unzip CLI 提取 DOCX (增强容错)
fn extract_docx_via_cli(path: &Path) -> Option<String> {
    let output = std::process::Command::new("unzip")
        .arg("-p")
        .arg(path)
        .arg("word/document.xml")
        .output()
        .ok()?;

    if output.status.success() {
        let xml = String::from_utf8_lossy(&output.stdout);
        parse_docx_xml(&xml)
    } else {
        None
    }
}

/// 解析 PDF 文本（优先利用 pdftotext CLI 工具）
fn extract_pdf_text(path: &Path) -> Option<String> {
    let output = std::process::Command::new("pdftotext")
        .arg("-q")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;

    if output.status.success() {
        let txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !txt.is_empty() {
            return Some(txt);
        }
    }
    None
}

/// 从参考文档内容与文件名中抽取专有术语/关键词，用于注入 Whisper ASR 的 Context Prompt
/// Whisper prompt 限制约 224 tokens（中文约 150-200 字符）
pub fn extract_glossary_for_asr(file_names: &[&str], doc_texts: &[&str]) -> String {
    use std::collections::BTreeSet;

    let mut terms = BTreeSet::new();

    // 1. 从文件名中提取词汇（例如："设备使用记录与主子设备同步方案.docx" -> "设备使用记录", "PRD"）
    for name in file_names {
        let base = Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name);
        for part in base.split(|c: char| !c.is_alphanumeric()) {
            let p = part.trim();
            if p.len() >= 2 && p.len() <= 25 {
                terms.insert(p.to_string());
            }
        }
    }

    // 2. 从正文中提取典型英数专有名词与高频核心词（如 PRD, API, K8s, OAuth2, GraphQL, 模块代号等）
    for doc in doc_texts {
        for word in doc.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
            let clean = word.trim();
            // 抓取全大写或驼峰英文缩写（如 PRD, OAuth2, GraphQL, ASR）
            if clean.len() >= 2
                && clean.len() <= 20
                && clean
                    .chars()
                    .any(|c| c.is_ascii_uppercase())
            {
                terms.insert(clean.to_string());
                if terms.len() >= 30 {
                    break;
                }
            }
        }
    }

    if terms.is_empty() {
        return String::new();
    }

    let mut prompt = String::from("讨论术语包含：");
    let mut current_len = prompt.chars().count();
    for t in terms {
        if current_len + t.chars().count() + 2 > 120 {
            break;
        }
        prompt.push_str(&t);
        prompt.push('，');
        current_len += t.chars().count() + 1;
    }
    prompt.trim_end_matches('，').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_check() {
        assert!(is_image_file(Path::new("screenshot.PNG")));
        assert!(is_image_file(Path::new("diagram.webp")));
        assert!(!is_image_file(Path::new("document.pdf")));

        assert!(is_document_file(Path::new("spec.docx")));
        assert!(is_document_file(Path::new("readme.md")));
        assert!(is_document_file(Path::new("manual.pdf")));
        assert!(!is_document_file(Path::new("recording.wav")));
    }

    #[test]
    fn test_plain_text_extraction() {
        let temp_file = std::env::temp_dir().join(format!("test_{}.md", uuid::Uuid::new_v4()));
        std::fs::write(&temp_file, "# 会议议题\n- 客户主设备与子设备数据同步\n").unwrap();

        let extracted = extract_document_text(&temp_file);
        assert!(extracted.is_some());
        assert!(extracted.unwrap().contains("客户主设备与子设备数据同步"));

        let _ = std::fs::remove_file(&temp_file);
    }

    #[test]
    fn test_extract_glossary_for_asr() {
        let names = ["设备管理系统_PRD_v2.0.docx", "API接口规范.pdf"];
        let docs = ["系统支持主从设备与 OAuth2 鉴权，支持 GraphQL 查询。"];

        let glossary = extract_glossary_for_asr(&names, &docs);
        assert!(glossary.contains("PRD"));
        assert!(glossary.contains("OAuth2") || glossary.contains("GraphQL"));
    }
}
