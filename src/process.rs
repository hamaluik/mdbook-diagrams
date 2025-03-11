use std::path::PathBuf;

use color_eyre::{
    Result,
    eyre::{WrapErr, eyre},
};
use mdbook::book::{Book, Chapter};
use mime::Mime;
use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};
use serde_json::json;
use ureq::Agent;

use super::{Config, DiagramOutputFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
enum DiagramType {
    Mermaid,
    PlantUml,
    Other(String),
}

pub fn process(mut book: Book, config: Config, renderer: &str) -> Result<Book> {
    let agent_config = Agent::config_builder()
        .timeout_global(config.kroki_timeout)
        .build();
    let agent: Agent = agent_config.into();

    let mut error: Option<color_eyre::eyre::Error> = None;
    book.for_each_mut(|item| {
        if error.is_some() {
            return;
        }

        if let mdbook::BookItem::Chapter(chapter) = item {
            if let Err(e) =
                process_chapter(chapter, &config, &agent, renderer).wrap_err_with(|| {
                    format!("Failed to process diagrams in chapter: {}", chapter.name)
                })
            {
                error = Some(e);
            }
        }
    });
    if let Some(error) = error {
        return Err(error);
    }

    Ok(book)
}

fn code_lang_diagram_type(lang: &CowStr, config: &Config) -> Option<DiagramType> {
    match lang {
        s if s.starts_with(format!("{}mermaid", config.language_prefix).as_str()) => {
            Some(DiagramType::Mermaid)
        }
        s if s.starts_with(format!("{}plantuml", config.language_prefix).as_str()) => {
            Some(DiagramType::PlantUml)
        }
        s if !config.language_prefix.is_empty()
            && s.starts_with(config.language_prefix.as_str()) =>
        {
            Some(DiagramType::Other(
                s.to_string()
                    .strip_prefix(&config.language_prefix)
                    .expect("can strip prefix")
                    .to_string(),
            ))
        }
        _ => None,
    }
}

fn process_chapter(
    chapter: &mut Chapter,
    config: &Config,
    agent: &Agent,
    renderer: &str,
) -> Result<()> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};

    // mini state machine for the current plantuml tag
    let mut diagram_type: Option<DiagramType> = None;
    let mut code_block_contents: Option<String> = None;

    let mut events = Vec::new();
    for event in Parser::new(&chapter.content) {
        let event = match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) => {
                diagram_type = code_lang_diagram_type(lang, config);
                if diagram_type.is_some() {
                    code_block_contents = Some("".to_owned());
                    None // eat the start of diagram code blocks
                } else {
                    Some(event)
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(diagram_type) = &diagram_type {
                    let code_block_contents = code_block_contents
                        .take()
                        .expect("can take code block contents");
                    process_diagram(
                        code_block_contents.as_str(),
                        diagram_type.clone(),
                        config,
                        agent,
                        renderer,
                        &mut events,
                    )
                    .wrap_err_with(|| {
                        format!("Failed to process diagram in chapter {}. Failing diagram:\n{code_block_contents}", chapter.name)
                    })?;
                    None // eat the end of diagram code blocks
                } else {
                    Some(event)
                }
            }
            Event::Text(ref txt) => {
                if let Some(code_block_contents) = code_block_contents.as_mut() {
                    code_block_contents.push_str(txt);
                    None // eat the text contents of the code block
                } else {
                    Some(event)
                }
            }
            // don't touch other events
            _ => Some(event),
        };

        if let Some(event) = event {
            events.push(event);
        }
    }

    let mut buf = String::with_capacity(chapter.content.len());
    pulldown_cmark_to_cmark::cmark(events.into_iter(), &mut buf).expect("can re-render cmark");
    chapter.content = buf;

    Ok(())
}

fn hash(diagram: &str, format: &DiagramOutputFormat, diagram_type: &DiagramType) -> String {
    use sha1::{Digest, Sha1};

    let mut hasher = Sha1::new();
    hasher.update(diagram.as_bytes());
    hasher.update(format.to_string().as_bytes());
    hasher.update(diagram_type.to_string().as_bytes());
    let result = hasher.finalize();

    let mut hash = String::new();
    for byte in result {
        hash.push_str(&format!("{:02x}", byte));
    }
    hash
}

fn get_filename(diagram: &str, diagram_type: &DiagramType, config: &Config) -> String {
    let hash = hash(diagram, &config.output_format, diagram_type);
    let Config {
        filename_prefix,
        output_format,
        ..
    } = config;
    format!("{filename_prefix}{hash}.{output_format}")
}

fn get_tmp_filepath(diagram: &str, diagram_type: &DiagramType, config: &Config) -> PathBuf {
    let filename = get_filename(diagram, diagram_type, config);
    config.files_path.join(filename)
}

fn fetch_from_tmp(
    diagram: &str,
    diagram_type: &DiagramType,
    config: &Config,
) -> Option<(PathBuf, Vec<u8>)> {
    let path = get_tmp_filepath(diagram, diagram_type, config);
    if path.exists() {
        let contents = std::fs::read(&path).ok()?;
        Some((path, contents))
    } else {
        None
    }
}

fn render_kroki(
    diagram: &str,
    diagram_type: DiagramType,
    config: &Config,
    agent: &Agent,
    renderer: &str,
) -> Result<(PathBuf, Vec<u8>)> {
    let Config {
        output_format,
        kroki_url,
        ..
    } = config;

    let mut diagram_options = json!({});
    if renderer != "html" {
        if let DiagramType::Mermaid = diagram_type {
            // html labels need to be disabled for non-html renderers otherwise
            // the svg won't show any text (see https://github.com/typst/typst/issues/1421)
            diagram_options["html-labels"] = "false".into();
        }
    }

    let req = json!({
        "diagram_source": diagram,
        "diagram_type": diagram_type.to_string(),
        "output_format": output_format.to_string(),
        "diagram_options": diagram_options
    });

    let mut response = agent
        .post(kroki_url)
        .header("Content-Type", "application/json")
        .send_json(req)
        .wrap_err_with(|| format!("Failed to send diagram to Kroki service at {kroki_url}"))?;

    let mime_type = response.headers().get("Content-Type");
    let output_format: DiagramOutputFormat = if let Some(mime_type) = mime_type {
        let mime_type = mime_type
            .to_str()
            .wrap_err("Failed to convert response mime type to string")?;
        let mime_type: Mime = mime_type.parse().wrap_err_with(|| {
            format!("Failed to parse response mime type as MIME type: {mime_type}",)
        })?;

        if mime_type == mime::IMAGE_SVG {
            DiagramOutputFormat::Svg
        } else if mime_type == mime::IMAGE_PNG {
            DiagramOutputFormat::Png
        } else {
            return Err(eyre!(
                "Unexpected response mime type from Kroki service: {mime_type} (expected image/svg+xml or image/png)"
            ));
        }
    } else {
        config.output_format
    };
    if output_format != config.output_format {
        return Err(eyre!(
            "Kroki service returned unexpected output format: {output_format} (expected {expected_output_format})",
            expected_output_format = config.output_format
        ));
    }

    let rendered_diagram = response
        .body_mut()
        .read_to_vec()
        .wrap_err("Failed to read diagram response")?;

    let path = get_tmp_filepath(diagram, &diagram_type, config);
    std::fs::write(&path, &rendered_diagram).wrap_err_with(|| {
        format!(
            "Failed to write rendered diagram to temporary file at {path}",
            path = path.display()
        )
    })?;

    Ok((path, rendered_diagram))
}

fn render(
    diagram: &str,
    diagram_type: DiagramType,
    config: &Config,
    agent: &Agent,
    renderer: &str,
) -> Result<(PathBuf, Vec<u8>)> {
    if let Some((path, contents)) = fetch_from_tmp(diagram, &diagram_type, config) {
        Ok((path, contents))
    } else {
        render_kroki(diagram, diagram_type, config, agent, renderer)
    }
}

fn process_diagram(
    diagram: &str,
    diagram_type: DiagramType,
    config: &Config,
    agent: &Agent,
    renderer: &str,
    events: &mut Vec<Event>,
) -> Result<()> {
    let (path, contents) = render(diagram, diagram_type, config, agent, renderer)
        .wrap_err_with(|| "Failed to render diagram")?;

    if renderer == "html" {
        match config.output_format {
            DiagramOutputFormat::Svg => {
                let svg: String = String::from_utf8(contents).expect("valid utf-8");
                let svg = svg.replace(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#,
                    "",
                );

                let event = Event::Html(CowStr::from(format!(
                    "<figure style='display: flex;flex-direction: row;justify-content: center;'>{svg}</figure>\n\n"
                )));
                events.push(event);
                Ok(())
            }
            DiagramOutputFormat::Png => {
                use base64::prelude::*;
                let b64 = BASE64_STANDARD.encode(&contents);
                let mime_type = config.output_format.mime_type();
                let uri = format!("data:{mime_type};base64,{b64}");

                let event = Event::Html(CowStr::from(format!(
                    "<figure style='display: flex;flex-direction: row;justify-content: center;'><img src=\"{uri}\" alt=\"rendered diagram\" /></figure>\n\n"
                )));
                events.push(event);
                Ok(())
            }
        }
    } else {
        let event_start = Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: CowStr::from(path.to_string_lossy().to_string()),
            title: "".into(),
            id: "".into(),
        });
        let event_end = Event::End(TagEnd::Image);

        events.push(event_start);
        events.push(event_end);
        events.push(Event::Text(CowStr::from("\n\n")));

        Ok(())
    }
}

impl std::fmt::Display for DiagramOutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagramOutputFormat::Svg => write!(f, "svg"),
            DiagramOutputFormat::Png => write!(f, "png"),
        }
    }
}

impl std::fmt::Display for DiagramType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagramType::Mermaid => write!(f, "mermaid"),
            DiagramType::PlantUml => write!(f, "plantuml"),
            DiagramType::Other(s) => write!(f, "{}", s.to_lowercase()),
        }
    }
}

impl DiagramOutputFormat {
    fn mime_type(&self) -> Mime {
        match self {
            DiagramOutputFormat::Svg => mime::IMAGE_SVG,
            DiagramOutputFormat::Png => mime::IMAGE_PNG,
        }
    }
}
