use deunicode::deunicode;
use once_cell::sync::Lazy;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::Scope;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

enum State<'a> {
    Heading {
        text: String,
    },
    CodeBlock {
        highlighter: ClassedHTMLGenerator<'a>,
    },
}

pub struct Transformer<'a, I> {
    events: I,
    next: Option<Event<'a>>,
    state: Option<State<'a>>,
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
                    let syntax = Scope::new(&format!("source.{}", lang))
                        .ok()
                        .and_then(|scope| SYNTAX_SET.find_syntax_by_scope(scope))
                        .or_else(|| SYNTAX_SET.find_syntax_by_name(lang))
                        .or_else(|| SYNTAX_SET.find_syntax_by_extension(lang));

                    if let Some(syntax) = syntax {
                        self.state = Some(State::CodeBlock {
                            highlighter: ClassedHTMLGenerator::new_with_class_style(
                                syntax,
                                &SYNTAX_SET,
                                ClassStyle::Spaced,
                            ),
                        });
                    }

                    return Some(event);
                }

                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                    if let Some(State::CodeBlock { highlighter }) = self.state.take() {
                        self.next = Some(event);
                        return Some(Event::Html(CowStr::Boxed(
                            highlighter.finalize().into_boxed_str(),
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
                        Some(State::CodeBlock { highlighter }) => {
                            for line in LinesWithEndings::from(text) {
                                highlighter.parse_html_for_line_which_includes_newline(line);
                            }

                            continue;
                        }
                        _ => {}
                    }

                    return Some(event);
                }

                Event::Code(ref code) => {
                    match &mut self.state {
                        Some(State::Heading { text: heading_text }) => {
                            heading_text.push_str(code);
                        }
                        _ => {}
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

fn create_anchor(s: &str) -> String {
    deunicode(s)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}
