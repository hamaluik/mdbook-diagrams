use std::{path::PathBuf, time::Duration};

use mdbook::{
    book::Book,
    errors::Error,
    preprocess::{Preprocessor, PreprocessorContext},
};

mod process;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
enum DiagramOutputFormat {
    #[default]
    Png,
    Svg,
}

#[derive(Debug)]
struct Config {
    output_format: DiagramOutputFormat,
    language_prefix: String,
    kroki_url: String,
    kroki_timeout: Option<Duration>,
    filename_prefix: String,
    files_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output_format: DiagramOutputFormat::Png,
            language_prefix: "".to_string(),
            kroki_url: "https://kroki.io".to_string(),
            kroki_timeout: None,
            filename_prefix: "diagram-".to_string(),
            files_path: std::env::temp_dir(),
        }
    }
}

#[derive(Debug, Default)]
pub struct DiagramsPreprocessor;

impl Preprocessor for DiagramsPreprocessor {
    fn name(&self) -> &str {
        "diagrams"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        let mut config = Config::default();
        if let Some(config_in) = ctx.config.get_preprocessor("diagrams") {
            if let Some(output_format) = config_in.get("output_format") {
                if let Some(output_format) = output_format.as_str() {
                    match output_format {
                        "png" => config.output_format = DiagramOutputFormat::Png,
                        "svg" => config.output_format = DiagramOutputFormat::Svg,
                        _ => {
                            return Err(Error::msg(format!(
                                "Invalid output_format: {}, expected 'png' or 'svg'",
                                output_format
                            )));
                        }
                    }
                }
            }

            if let Some(language_prefix) = config_in.get("language_prefix") {
                if let Some(language_prefix) = language_prefix.as_str() {
                    config.language_prefix = language_prefix.to_string();
                }
            }

            if let Some(kroki_url) = config_in.get("kroki_url") {
                if let Some(kroki_url) = kroki_url.as_str() {
                    config.kroki_url = kroki_url.to_string();
                }
            }

            if let Some(kroki_timeout_secs) = config_in.get("kroki_timeout_secs") {
                if let Some(kroki_timeout_secs) = kroki_timeout_secs.as_float() {
                    config.kroki_timeout = Some(Duration::from_secs_f64(kroki_timeout_secs));
                }
            }

            if let Some(filename_prefix) = config_in.get("filename_prefix") {
                if let Some(filename_prefix) = filename_prefix.as_str() {
                    config.filename_prefix = filename_prefix.to_string();
                }
            }

            if let Some(files_path) = config_in.get("files_path") {
                if let Some(files_path) = files_path.as_str() {
                    if !files_path.is_empty() {
                        config.files_path = PathBuf::from(files_path);
                        std::fs::create_dir_all(&config.files_path).map_err(Error::msg)?;
                    }
                }
            }
        }

        let book = process::process(book, config, &ctx.renderer).map_err(Error::msg)?;
        Ok(book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn render_svg_for_html() {
        let input_json = r##"[
        {
            "root": "/path/to/book",
            "config": {
                "book": {
                    "authors": ["AUTHOR"],
                    "language": "en",
                    "multilingual": false,
                    "src": "src",
                    "title": "TITLE"
                },
                "preprocessor": {
                    "diagrams": {
                        "output_format": "svg",
                        "language_prefix": "",
                        "kroki_url": "https://kroki.io",
                        "kroki_timeout_secs": 5.0,
                        "filename_prefix": "diagram-",
                        "files_path": null
                    }
                }
            },
            "renderer": "html",
            "mdbook_version": "0.4.21"
        },
        {
            "sections": [
            {
                "Chapter": {
                    "name": "Chapter 1",
                    "content": "# Chapter 1\n```mermaid\nsequenceDiagram\n    Alice ->> Bob: Hello Bob, how are you?\n    Bob-->>John: How about you John?\n    Bob--x Alice: I am good thanks!\n    Bob-x John: I am good thanks!\n    Note right of John: Bob thinks a long<br/>long time, so long<br/>that the text does<br/>not fit on a row.\n\n    Bob-->Alice: Checking with John...\n    Alice->John: Yes... John, how are you?\n```",
                    "number": [1],
                    "sub_items": [],
                    "path": "chapter_1.md",
                    "source_path": "chapter_1.md",
                    "parent_names": []
                }
            }
            ],
            "__non_exhaustive": null
        }
        ]"##;
        let input_json = input_json.as_bytes();

        let (ctx, book) = mdbook::preprocess::CmdPreprocessor::parse_input(input_json).unwrap();
        let result = DiagramsPreprocessor.run(&ctx, book);
        assert!(result.is_ok());

        let mut output = String::new();
        let has_svg = result.unwrap().sections.iter().any(|item| match item {
            mdbook::book::BookItem::Chapter(chapter) => {
                output.push_str(&chapter.content);
                chapter.content.contains("<svg")
            }
            _ => false,
        });
        assert!(has_svg, "Expected SVG in output: {output}");
    }

    #[test]
    fn render_image_for_other() {
        let input_json = r##"[
        {
            "root": "/path/to/book",
            "config": {
                "book": {
                    "authors": ["AUTHOR"],
                    "language": "en",
                    "multilingual": false,
                    "src": "src",
                    "title": "TITLE"
                },
                "preprocessor": {
                    "diagrams": {
                        "output_format": "png",
                        "language_prefix": "",
                        "kroki_url": "https://kroki.io",
                        "kroki_timeout_secs": 5.0,
                        "filename_prefix": "diagram-",
                        "files_path": null
                    }
                }
            },
            "renderer": "pandoc",
            "mdbook_version": "0.4.21"
        },
        {
            "sections": [
            {
                "Chapter": {
                    "name": "Chapter 1",
                    "content": "# Chapter 1\n```mermaid\nsequenceDiagram\n    Alice ->> Bob: Hello Bob, how are you?\n    Bob-->>John: How about you John?\n    Bob--x Alice: I am good thanks!\n    Bob-x John: I am good thanks!\n    Note right of John: Bob thinks a long<br/>long time, so long<br/>that the text does<br/>not fit on a row.\n\n    Bob-->Alice: Checking with John...\n    Alice->John: Yes... John, how are you?\n```",
                    "number": [1],
                    "sub_items": [],
                    "path": "chapter_1.md",
                    "source_path": "chapter_1.md",
                    "parent_names": []
                }
            }
            ],
            "__non_exhaustive": null
        }
        ]"##;
        let input_json = input_json.as_bytes();

        let (ctx, book) = mdbook::preprocess::CmdPreprocessor::parse_input(input_json).unwrap();
        let result = DiagramsPreprocessor.run(&ctx, book);
        assert!(result.is_ok());

        let mut output = String::new();
        let tmp_path = std::env::temp_dir();
        let tmp_path = tmp_path.to_str().expect("can get temp dir");
        let has_svg = result.unwrap().sections.iter().any(|item| match item {
            mdbook::book::BookItem::Chapter(chapter) => {
                output.push_str(&chapter.content);
                chapter
                    .content
                    .contains(&format!("![]({tmp_path}/diagram-")) // t
            }
            _ => false,
        });
        assert!(has_svg, "Expected image link in output: {output}");
    }
}
