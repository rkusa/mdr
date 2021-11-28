use std::collections::HashMap;

use deunicode::deunicode;
use once_cell::sync::Lazy;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, Highlighter, HtmlRenderer};

enum State {
    Heading {
        text: String,
    },
    CodeBlock {
        lang: String,
        renderer: HtmlRenderer,
    },
}

pub struct Transformer<'a, I> {
    events: I,
    next: Option<Event<'a>>,
    state: Option<State>,
    meta: String,
    title: Option<String>,
}

impl<'a, I> Transformer<'a, I> {
    pub fn new(events: I) -> Self {
        Self {
            events,
            next: None,
            state: None,
            meta: String::new(),
            title: None,
        }
    }

    pub fn meta(&self) -> &str {
        &self.meta
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

impl<'a, I> Iterator for Transformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next.take() {
            return Some(next);
        }

        for event in &mut self.events {
            // dbg!(&event);

            match event {
                Event::Start(Tag::Heading(_)) => {
                    self.state = Some(State::Heading {
                        text: String::new(),
                    });

                    return Some(event);
                }

                Event::End(Tag::Heading(_)) => {
                    if let Some(State::Heading { text }) = self.state.take() {
                        let id = create_anchor(&text); // TODO: avoid duplicates
                        let anchor = format!(
                            r##"<a href="#{}" id="{}" class="anchor" aria-hidden="true" tabindex="-1">{}</a>"##,
                            id,
                            id,
                            include_str!("theme/link.svg"),
                        );

                        if self.title.is_none() {
                            self.title = Some(text);
                        }

                        self.next = Some(event);

                        return Some(Event::Html(CowStr::Boxed(anchor.into_boxed_str())));
                    } else {
                        return Some(event);
                    };
                }

                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) => {
                    if !lang.is_empty() && get_highlight_config(lang).is_some() {
                        self.state = Some(State::CodeBlock {
                            lang: lang.to_string(),
                            renderer: HtmlRenderer::new(),
                        });
                    }
                    return Some(event);
                }

                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                    if let Some(State::CodeBlock { renderer, .. }) = self.state.take() {
                        self.next = Some(event);
                        return Some(Event::Html(CowStr::Boxed(
                            renderer.lines().collect::<String>().into_boxed_str(),
                        )));
                    } else {
                        return Some(event);
                    }
                }

                Event::Text(ref text) => {
                    match &mut self.state {
                        Some(State::Heading { text: heading_text }) => {
                            heading_text.push_str(text);
                        }
                        Some(State::CodeBlock { lang, renderer }) => {
                            if let Some(config) = get_highlight_config(lang) {
                                let mut highlighter = Highlighter::new();
                                let highlights = highlighter
                                    .highlight(config, text.as_bytes(), None, |lang| {
                                        get_highlight_config(lang)
                                    })
                                    .unwrap();

                                renderer
                                    .render(highlights, text.as_bytes(), &highlight_class_name)
                                    .unwrap();
                            }

                            continue;
                        }
                        _ => {}
                    }

                    return Some(event);
                }

                Event::Code(ref code) => {
                    if let Some(State::Heading { text: heading_text }) = &mut self.state {
                        heading_text.push_str(code);
                    }

                    return Some(event);
                }

                // extract <meta /> tags to move them into the head of the document
                Event::Html(html) if html.starts_with("<meta ") => {
                    self.meta += &html;
                    continue;
                }

                _ => return Some(event),
            }
        }

        None
    }
}

const HIGHLIGHT_NAMES: &[&str] = &[
    "comment",
    "punctuation",
    "string",
    "type",
    // "attribute",
    // "constant",
    // "function.builtin",
    // "function",
    // "keyword",
    // "operator",
    // "property",
    // "punctuation.bracket",
    // "punctuation.delimiter",
    // "string.special",
    // "tag",
    // "type.builtin",
    // "variable",
    // "variable.builtin",
    // "variable.parameter",
];

static HIGHLIGHT_CONFIGS: Lazy<HashMap<&'static str, HighlightConfiguration>> = Lazy::new(|| {
    let mut configs = HashMap::with_capacity(1);
    configs.insert("js", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_javascript::language(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTION_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs.insert("jsx", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_javascript::language(),
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTION_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs.insert("ts", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_typescript::language_typescript(),
            tree_sitter_typescript::HIGHLIGHT_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs.insert("tsx", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_typescript::language_tsx(),
            tree_sitter_typescript::HIGHLIGHT_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs.insert("rs", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_rust::language(),
            tree_sitter_rust::HIGHLIGHT_QUERY,
            "",
            "",
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs.insert("bash", {
        let mut config = HighlightConfiguration::new(
            tree_sitter_bash::language(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        )
        .unwrap();
        config.configure(HIGHLIGHT_NAMES);
        config
    });
    configs
});

static HIGHLIGHT_ATTRS: Lazy<Vec<String>> = Lazy::new(|| {
    HIGHLIGHT_NAMES
        .iter()
        .map(|scope| format!(r#"class="{}""#, scope.replace('.', " ")))
        .collect()
});

fn get_highlight_config(lang: &str) -> Option<&'static HighlightConfiguration> {
    match lang.to_ascii_lowercase().as_str() {
        "js" | "javascript" => HIGHLIGHT_CONFIGS.get("js"),
        "jsx" => HIGHLIGHT_CONFIGS.get("jsx"),
        "ts" | "typescript" => HIGHLIGHT_CONFIGS.get("ts"),
        "tsx" => HIGHLIGHT_CONFIGS.get("tsx"),
        "rs" | "rust" => HIGHLIGHT_CONFIGS.get("rs"),
        "bash" => HIGHLIGHT_CONFIGS.get("bash"),
        _ => {
            log::warn!(
                "cannot highlight unsupported code block language `{}`",
                lang
            );
            None
        }
    }
}

fn highlight_class_name<'a>(highlight: Highlight) -> &'a [u8] {
    HIGHLIGHT_ATTRS[highlight.0].as_bytes()
}

fn create_anchor(s: &str) -> String {
    deunicode(s)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}
