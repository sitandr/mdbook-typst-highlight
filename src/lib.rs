use anyhow::anyhow;
use syntect::highlighting::Color;
use mdbook::book::Book;
use mdbook::errors::{Error, Result};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::utils::new_cmark_parser;
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag};
use pulldown_cmark_to_cmark::cmark;

use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSetBuilder;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground, append_highlighted_html_for_styled_line};
use syntect::util::LinesWithEndings;


pub struct TypstHighlight;


impl Preprocessor for TypstHighlight {
    fn name(&self) -> &str {
        "typst-highlight"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        
        let highlight_inline = if let Some(typst_cfg) = ctx.config.get_preprocessor(self.name()) {
            !typst_cfg.contains_key("disable_inline")
        } else {
            true
        };

        book.sections
            .iter_mut()
            .try_for_each(|section| process_chapter(section, highlight_inline))?;

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn process_chapter(section: &mut BookItem, highlight_inline: bool) -> Result<()> {
    if let BookItem::Chapter(chapter) = section {

        chapter.sub_items.iter_mut().try_for_each(|section| process_chapter(section, highlight_inline))?;

        let events = new_cmark_parser(&chapter.content, false);

        let mut new_events = Vec::new();

        let mut codeblock_text = None;

        for event in events {
            match event {
                Event::Start(tag) => {
                    if is_typst_codeblock(&tag) {
                        codeblock_text = Some(String::new())
                    }
                    else {
                        new_events.push(Event::Start(tag))
                    }
                }
                Event::End(tag) => {
                    if is_typst_codeblock(&tag) {
                        new_events.push(Event::Html(highlight(
                            codeblock_text
                                .ok_or(anyhow!("Typst codeblock wasn't created"))?
                                .into(),
                            false
                        )?));
                        // new_events.push(Event::SoftBreak);
                        codeblock_text = None
                    }
                    else {
                        new_events.push(Event::End(tag))
                    }
                }
                Event::Code(code) if highlight_inline => {
                    new_events.push(Event::Html(highlight(code, true)?))
                }
                Event::Text(s) => {
                    if let Some(ref mut text) = codeblock_text {
                        text.push_str(&s)
                    }
                    else {
                        new_events.push(Event::Text(s))
                    }
                }
                ev => new_events.push(ev),
            }
        }

        let mut buf = String::with_capacity(chapter.content.len());
        cmark(new_events.into_iter(), &mut buf).map_err(|err| {
            anyhow!("Markdown serialization failed: {}", err)
        })?;
        chapter.content = buf;
        // chapter.sub_items.iter_mut().for_each(|item| {item.clone()});
    }
    Ok(())
}

fn is_typst_codeblock(t: &Tag) -> bool {
    if let Tag::CodeBlock(ref kind) = *t {
        match kind {
            CodeBlockKind::Fenced(kind) => kind.as_ref() == "typ" || kind.as_ref() == "typst",
            CodeBlockKind::Indented => true,
        }
    } else {
        false
    }
}

fn highlight(s: CowStr, inline: bool) -> Result<CowStr> {
    let mut s = s.into_string();
    if s.chars().last() == Some('\n') {
        s.pop();
    }

    let ts = ThemeSet::load_defaults();
    let mut theme = ts.themes["Solarized (dark)"].clone();
    // eprintln!("{theme:?}");

    theme.settings.background = Some(Color{r: 32, g: 32, b: 32, a: 0});
    theme.settings.foreground = Some(Color{r: 27, g: 223, b: 51, a: 99});
    // The probality that the hack will break when you are writing colors is ≈ 1/(2⁸)⁴ ≈ 1/(2³²)
    // In fact much less, very few people use alphas

    let typst_syntax = syntect::parsing::syntax_definition::SyntaxDefinition::load_from_str(include_str!("../res/Typst.sublime-syntax"), true, None)?;
    let mut syntax = SyntaxSetBuilder::new();
    syntax.add(typst_syntax);
    let syntax_set = syntax.build();
    let syntax = syntax_set.syntaxes().first().unwrap();
    let mut html = if inline {
        let mut h = HighlightLines::new(syntax, &theme);
        let regs = h.highlight_line(s.as_ref(), &syntax_set)?;
        let html = styled_line_to_highlighted_html(&regs[..], IncludeBackground::No)?;
        format!(r#"<code class="hljs">{}</code>"#, html)
    }
    else {
        let mut html = r#"<pre style="margin: 0"><code class="language-typ hljs">"#.into();
        
        let mut highlighter = HighlightLines::new(syntax, &theme);

        for line in LinesWithEndings::from(&s) {
            let regions = highlighter.highlight_line(line, &syntax_set)?;
            append_highlighted_html_for_styled_line(
                &regions[..],
                IncludeBackground::No,
                &mut html,
            )?;
        }

        html.push_str("</code></pre>\n");

        html
    };

    html = html.replace("#1bdf3363", "var(--fg)");

    Ok(html.into())
}
