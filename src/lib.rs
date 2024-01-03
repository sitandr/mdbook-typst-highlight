use async_process::Command;
use futures::future::join_all;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;

use anyhow::anyhow;
use lazy_static::lazy_static;
use mdbook::book::Book;
use mdbook::errors::{Error, Result};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::utils::new_cmark_parser;
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag};
use pulldown_cmark_to_cmark::cmark;
use syntect::highlighting::Color;
use syntect::parsing::SyntaxSet;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{
    append_highlighted_html_for_styled_line, styled_line_to_highlighted_html, IncludeBackground,
};
use syntect::util::LinesWithEndings;

static PREAMBLE: &str = "
#set page(height: auto, width: 400pt, margin: 0.5cm)
";

lazy_static! {
    /// This is an example for using doc comment attributes
    static ref THEME: Theme = {
        let ts = ThemeSet::load_defaults();
        let mut theme = ts.themes["Solarized (dark)"].clone();
        theme.settings.foreground = Some(Color {
            r: 27,
            g: 223,
            b: 51,
            a: 99,
        });
        // The probality that the hack will break when you are writing colors is ≈ 1/(2⁸)⁴ ≈ 1/(2³²)
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

struct PreprocessSettings {
    highlight_inline: bool,
    typst_default: bool,
    render: bool,
    warn_not_specified: bool
}

pub struct TypstHighlight;

fn get_setting(preprocessor: Option<&toml::map::Map<String, toml::Value>>, name: &str) -> bool {
    preprocessor
        .and_then(|typst_cfg| {
            typst_cfg
                .get(name)
                .map(|v| v.as_bool().expect(&("Incorrect argument at".to_owned() + name)))
        })
        .unwrap_or(false)
}

impl Preprocessor for TypstHighlight {
    fn name(&self) -> &str {
        "typst-highlight"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let prep = ctx.config.get_preprocessor(self.name());

        let highlight_inline = !get_setting(prep, "disable_inline");
        let typst_default = get_setting(prep, "typst_default");
        let render = get_setting(prep, "render");
        let warn_not_specified = get_setting(prep, "warn_not_specified");

        let settings = PreprocessSettings{ highlight_inline, typst_default, render, warn_not_specified };

        book.sections.iter_mut().try_for_each(|section| {
            let mut build_dir = ctx.root.clone();
            build_dir.push(&ctx.config.book.src);

            process_chapter(section, &settings, &build_dir)
        })?;

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn process_chapter(
    section: &mut BookItem,
    settings: &PreprocessSettings,
    build_dir: &PathBuf,
) -> Result<()> {
    if let BookItem::Chapter(chapter) = section {
        chapter.sub_items.iter_mut().try_for_each(|section| {
            process_chapter(section, settings, build_dir)
        })?;

        let events = new_cmark_parser(&chapter.content, false);
        let mut new_events = Vec::new();
        let mut codeblock_text = None;

        let mut chapter_path = build_dir.clone();
        if let Some(p) = chapter.path.as_ref().and_then(|p| p.parent()) {
            chapter_path.push(p)
        };

        let mut compile_errors = vec![];

        for event in events {
            match event {
                Event::Start(tag) => {
                    let lang = get_lang(&tag, settings, None);

                    if let Some(lang) = lang {
                        if is_typst_codeblock(lang) {
                            codeblock_text = Some(String::new())
                        } else {
                            new_events.push(Event::Start(tag))
                        }
                    } else {
                        new_events.push(Event::Start(tag))
                    }
                }
                Event::End(tag) => {
                    let lang = get_lang(&tag, settings, Some(&chapter.name));

                    if let Some(lang) = lang {
                        if is_typst_codeblock(lang) {
                            let text = codeblock_text.ok_or(anyhow!(
                                "Typst codeblock wasn't created: chapter {}.
                                    Data collected: {:?}",
                                chapter.name,
                                new_events
                            ))?;

                            let mut html = highlight(text.clone().into(), false);

                            if settings.render && !lang.contains("norender") {
                                let (file, err) = render_block(
                                    text,
                                    chapter_path.clone(),
                                    build_dir.clone(),
                                    chapter.name.clone(),
                                    !lang.contains("nopreamble"),
                                );

                                compile_errors.extend(err);

                                html += format!(
                                    r#"<div style="
                                    text-align: center;
                                    padding: 0.5em;
                                    background: var(--quote-bg);
                                    "><img align="middle" src="typst-img/{file}.svg" alt="Rendered image" style="
                                    background: white;
                                    max-width: 500pt;
                                    width: 100%;
                                "></div>"#
                                ).as_str();
                            }
                            new_events.push(Event::Html(
                                format!(r#"<div style="margin-bottom: 0.5em">{}</div>"#, html)
                                    .into(),
                            ));
                            new_events.push(Event::HardBreak);
                            codeblock_text = None
                        } else {
                            new_events.push(Event::End(tag))
                        }
                    } else {
                        new_events.push(Event::End(tag))
                    }
                }
                Event::Code(code) if settings.highlight_inline => {
                    new_events.push(Event::Html(highlight(code, true).into()))
                }
                Event::Text(s) => {
                    if let Some(ref mut text) = codeblock_text {
                        text.push_str(&s)
                    } else {
                        new_events.push(Event::Text(s))
                    }
                }
                ev => new_events.push(ev),
            }
        }

        let mut buf = String::with_capacity(chapter.content.len());
        cmark(new_events.into_iter(), &mut buf)
            .map_err(|err| anyhow!("Markdown serialization failed: {}", err))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(async { join_all(compile_errors).await });

        chapter.content = buf;
    }
    Ok(())
}

fn get_lang<'a>(t: &'a Tag, settings: &PreprocessSettings, chapter: Option<&str>) -> Option<&'a str> {
    let default = if settings.typst_default {
        Some("typ")
    } else {
        None
    };
    if let Tag::CodeBlock(ref kind) = *t {
        match kind {
            CodeBlockKind::Fenced(kind) => (!kind.is_empty()).then(|| kind.as_ref())
                .or_else(|| {
                    if settings.warn_not_specified {
                        if let Some(chapter) = chapter {
                            eprintln!("Codeblock language not specified in {}", chapter)
                        }
                    }
                    default
                }),
            CodeBlockKind::Indented => default
        }
    } else {
        None
    }
}

fn is_typst_codeblock(s: &str) -> bool {
    s.contains("typ") || s.contains("typst")
}

fn highlight(s: CowStr, inline: bool) -> String {
    let mut s = s.into_string();
    if s.ends_with('\n') {
        s.pop();
    }

    let syntax = SYNTAX.syntaxes().last().unwrap();

    let mut html = if inline {
        let mut h = HighlightLines::new(syntax, &THEME);
        let regs = h.highlight_line(s.as_ref(), &SYNTAX).unwrap(); // everything should be fine
        let html = styled_line_to_highlighted_html(&regs[..], IncludeBackground::No).unwrap();
        format!(r#"<code class="hljs">{}</code>"#, html)
    } else {
        let mut html = r#"<pre style="margin: 0"><code class="language-typ hljs">"#.into();

        let mut highlighter = HighlightLines::new(syntax, &THEME);

        for line in LinesWithEndings::from(&s) {
            let regions = highlighter.highlight_line(line, &SYNTAX).unwrap();
            append_highlighted_html_for_styled_line(
                &regions[..],
                IncludeBackground::No,
                &mut html,
            ).unwrap();
        }

        html.push_str("</code></pre>\n");

        html
    };

    html = html.replace("#1bdf3363", "var(--fg)");

    html
}

fn sha256_hash(input: &str) -> String {
    let mut res = Sha256::new();
    res.update(input.as_bytes());
    let res = res.finalize();
    format!("{:x}", res)
}

fn render_block(
    src: String,
    mut dir: PathBuf,
    mut build_dir: PathBuf,
    name: String,
    preamble: bool,
) -> (String, Option<impl Future<Output = ()>>) {
    let filename = sha256_hash(&src);
    let mut output = dir.clone();
    output.push("typst-img");
    output.push(filename.clone() + ".svg");

    let mut command = None;

    if !output.exists() {
        fs::create_dir_all(&output.parent().unwrap()).expect("Can't create a dir");
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

    (filename, command)
}
