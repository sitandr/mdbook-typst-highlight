use async_process::Command;
use futures::future::join_all;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::future::Future;
use std::io::Write;
use std::iter;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::anyhow;
use lazy_static::lazy_static;
use mdbook_markdown::new_cmark_parser;
use mdbook_markdown::pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use mdbook_preprocessor::book::{Book, Chapter};
use mdbook_preprocessor::errors::{Error, Result};
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use pulldown_cmark_to_cmark::cmark;
use serde::Deserialize;
use syntect::highlighting::Color;
use syntect::parsing::SyntaxSet;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{
    append_highlighted_html_for_styled_line, styled_line_to_highlighted_html, IncludeBackground,
};
use syntect::util::LinesWithEndings;

static PREAMBLE: &str = "#set page(height: auto, width: 400pt, margin: 0.5cm)\n";

lazy_static! {
    static ref THEME: Theme = {
        let ts = ThemeSet::load_defaults();
        let mut theme = ts.themes["Solarized (dark)"].clone();
        theme.settings.foreground = Some(Color {
            r: 27,
            g: 223,
            b: 51,
            a: 99,
        });
        // The probability that the hack will break when you are writing colors is ≈ 1/(2⁸)⁴ ≈ 1/(2³²)
        // In fact much less, very few people use alphas

        theme
    };

    static ref SYNTAX: SyntaxSet = {
        let typst_syntax = syntect::parsing::syntax_definition::SyntaxDefinition::load_from_str(
            include_str!("../res/Typst.sublime-syntax"),
            true,
            None,
        ).expect("Syntax data was corrupted");

        let mut syntax = SyntaxSet::load_defaults_nonewlines().into_builder();
        syntax.add(typst_syntax);
        syntax.build()
    };
}

pub struct TypstHighlight;

#[derive(Deserialize, Default)]
struct PreprocessSettings {
    #[serde(default)]
    disable_inline: bool,
    #[serde(default)]
    typst_default: bool,
    #[serde(default)]
    render: bool,
    #[serde(default)]
    warn_not_specified: bool,
}

impl PreprocessSettings {
    #[inline(always)]
    fn highlight_inline(&self) -> bool {
        !self.disable_inline
    }
}

impl Preprocessor for TypstHighlight {
    fn name(&self) -> &str {
        "typst-highlight"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let settings = ctx
            .config
            .get::<PreprocessSettings>("preprocessor.typst-highlight")?
            .unwrap_or_default();

        let mut errors = vec![];

        book.for_each_chapter_mut(|chapter| {
            let mut build_dir = ctx.root.clone();
            build_dir.push(&ctx.config.book.src);

            if let Err(e) = process_chapter(chapter, &settings, &build_dir) {
                errors.push(e);
            }
        });

        if errors.is_empty() {
            Ok(book)
        } else {
            Err(anyhow!(
                "Errors occurred during preprocessing:\n{:#?}",
                errors
            ))
        }
    }

    fn supports_renderer(&self, renderer: &str) -> Result<bool> {
        Ok(renderer == "html")
    }
}

fn process_chapter(
    chapter: &mut Chapter,
    settings: &PreprocessSettings,
    build_dir: &Path,
) -> Result<()> {
    let events = new_cmark_parser(&chapter.content, &Default::default());
    let mut new_events = Vec::new();

    // (lang, text) of the current codeblock
    let mut current_codeblock: Option<(String, String)> = None;

    let mut chapter_path = build_dir.to_path_buf();
    if let Some(p) = chapter.path.as_ref().and_then(|p| p.parent()) {
        chapter_path.push(p)
    };

    let mut compile_errors = vec![];

    for event in events {
        match event {
            Event::Start(Tag::CodeBlock(ref kind)) => {
                match codeblock_lang(kind, settings, chapter.name.as_str()) {
                    Some(lang) if is_typst_codeblock(lang) => {
                        current_codeblock = Some((lang.to_owned(), String::new()))
                    }
                    _ => new_events.push(event),
                }
            }
            Event::End(TagEnd::CodeBlock) => match current_codeblock {
                Some((lang, text)) => {
                    let mut html = highlight(text.as_str(), false);

                    if settings.render && !lang.contains("norender") {
                        let (file, err) = render_block(
                            text,
                            chapter_path.clone(),
                            build_dir.to_path_buf(),
                            chapter.name.clone(),
                            !lang.contains("nopreamble"),
                        );
                        let file = file.to_str().unwrap();

                        compile_errors.extend(err);

                        html += format!("<typst-render-insert-image-{file}>").as_str();
                    }
                    new_events.push(Event::Start(Tag::HtmlBlock));
                    new_events.push(Event::Html(
                        format!(r#"<div style="margin-bottom: 0.5em">{}</div>"#, html).into(),
                    ));
                    new_events.push(Event::End(TagEnd::HtmlBlock));
                    new_events.push(Event::HardBreak);
                    current_codeblock = None
                }
                None => new_events.push(event),
            },
            Event::Code(code) if settings.highlight_inline() => {
                new_events.push(Event::InlineHtml(highlight(code.as_ref(), true).into()))
            }
            Event::Text(ref s) => match current_codeblock {
                Some((_, ref mut text)) => {
                    text.push_str(s);
                }
                None => new_events.push(event),
            },
            ev => new_events.push(ev),
        }
    }

    let runtime = tokio::runtime::Builder::new_current_thread().build()?;

    runtime.block_on(async { join_all(compile_errors).await });

    // Okay, all images are rendered now, so it's time to replace file names with true ones!

    let new_events = new_events.into_iter().map(|e| match e {
            Event::Html(s) if s.contains("<typst-render-insert-image-") => {
                const PATTLENGTH: usize = "<typst-render-insert-image-".len();

                let start = s.find("<typst-render-insert-image-").unwrap();
                let end = start
                    + PATTLENGTH
                    + s[start + PATTLENGTH..]
                        .find('>')
                        .expect("Someone who inserts crazy tags forgot to close the bracket");
                let file = PathBuf::from_str(&s[start + PATTLENGTH..end])
                    .expect("Problem when decoding path");

                let inner = get_images(file)
                    .map(|name| {
                        format!(
                            r#"<div style="text-align: center; padding: 0.5em; background: var(--quote-bg);">
                            <img align="middle" src="typst-img/{name}" alt="Rendered image" style="background: white; max-width: 500pt; width: 100%;">
                            </div>"#
                        )
                    })
                    .collect::<String>();

                let new_s = s[..start].to_owned() + inner.as_str() + &s[end + 1..];

                Event::Html(new_s.into())
            }
            e => e,
        });

    let mut buf = String::with_capacity(chapter.content.len());
    cmark(new_events.into_iter(), &mut buf)
        .map_err(|err| anyhow!("Markdown serialization failed: {}", err))?;

    chapter.content = buf;

    Ok(())
}

fn codeblock_lang<'a>(
    kind: &'a CodeBlockKind,
    settings: &PreprocessSettings,
    chapter: &str,
) -> Option<&'a str> {
    let default = if settings.typst_default {
        Some("typ")
    } else {
        None
    };
    match kind {
        CodeBlockKind::Fenced(kind) => {
            if !kind.is_empty() {
                Some(kind.as_ref())
            } else {
                if settings.warn_not_specified {
                    eprintln!("Codeblock language not specified in {}", chapter)
                }
                default
            }
        }
        CodeBlockKind::Indented => default,
    }
}

fn is_typst_codeblock(s: &str) -> bool {
    s.contains("typ") || s.contains("typst")
}

fn highlight(src: &str, inline: bool) -> String {
    let src = src.strip_suffix('\n').unwrap_or(src);

    let syntax = SYNTAX.syntaxes().last().unwrap();

    let mut html = if inline {
        let mut h = HighlightLines::new(syntax, &THEME);
        let regs = h.highlight_line(src, &SYNTAX).unwrap(); // everything should be fine
        let html = styled_line_to_highlighted_html(&regs[..], IncludeBackground::No).unwrap();
        format!(r#"<code class="hljs">{}</code>"#, html)
    } else {
        let mut html = r#"<pre style="margin: 0"><code class="language-typ hljs">"#.into();

        let mut highlighter = HighlightLines::new(syntax, &THEME);

        for line in LinesWithEndings::from(src) {
            let regions = highlighter.highlight_line(line, &SYNTAX).unwrap();
            append_highlighted_html_for_styled_line(&regions[..], IncludeBackground::No, &mut html)
                .unwrap();
        }

        html.push_str("</code></pre>");

        html
    };

    html = html.replace("#1bdf3363", "var(--fg)");

    html
}

fn sha256_hash(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    format!("{:x}", hash)
}

fn get_images(src: PathBuf) -> impl Iterator<Item = String> {
    let mut n = 1;
    let fbase = src.file_name().unwrap().to_str().unwrap().to_owned();

    iter::from_fn(move || {
        let path = src.clone();
        let path = path.with_file_name(fbase.clone() + format!("-{n}.svg").as_str());

        if path.exists() {
            n += 1;
            Some(path.file_name().unwrap().to_string_lossy().into_owned())
        } else {
            None
        }
    })
    .fuse()
}

fn render_block(
    src: String,
    mut dir: PathBuf,
    mut build_dir: PathBuf,
    name: String,
    preamble: bool,
) -> (PathBuf, Option<impl Future<Output = ()>>) {
    let filename = sha256_hash(&src);
    let mut output = dir.clone();
    output.push("typst-img");

    let mut check = output.clone();
    let mut cut_output = output.clone();
    cut_output.push(filename.clone());

    output.push(filename.clone() + "-{n}.svg");
    check.push(filename.clone() + "-1.svg");

    let mut command = None;

    if !check.exists() {
        fs::create_dir_all(output.parent().unwrap()).expect("Can't create a dir");
        dir.push("typst-src");
        fs::create_dir_all(&dir).expect("Can't create a dir");
        dir.push(filename.clone() + ".typ");

        let mut file = File::create(&dir).expect("Can't create file");
        if preamble {
            writeln!(file, "{}", PREAMBLE).expect("Error writing to file")
        };
        write!(file, "{}", src).expect("Error writing to file");

        let mut res = Command::new("typst");
        let mut res = res
            .arg("c")
            .arg(&dir)
            .arg("--root")
            .arg(dir.parent().unwrap().parent().unwrap())
            .arg(&output);

        build_dir.push("fonts");

        if build_dir.exists() {
            res = res.arg("--font-path").arg(build_dir)
        }

        let res = res.output();

        command = Some(async move {
            let output = res.await.expect("Failed").stderr;

            if !output.is_empty() {
                let stderr = std::io::stderr();
                let mut handle = stderr.lock();
                writeln!(handle, "Error at chapter \"{}\"\n", name).expect("Can't write to stderr");
                handle.write_all(&output).expect("Can't write to stderr");
            }
        });
    }

    (cut_output, command)
}
