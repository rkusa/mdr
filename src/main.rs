mod config;
mod transform;

use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

use crate::config::CONFIG;
use crate::transform::Transformer;
use chrono::{DateTime, NaiveDate, Utc};
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, ElementContentHandlers, RewriteStrSettings, Selector};
use pulldown_cmark::{html, Options, Parser};

fn main() -> Result<(), Error> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let out_dir = CONFIG.out_dir();
    fs::create_dir_all(out_dir)?;

    let mut css_path = PathBuf::new();
    css_path.push(out_dir);
    css_path.push("style.css");
    fs::write(css_path, include_str!("theme/style.css"))?;

    let mut posts = Vec::new();

    for path in CONFIG.files() {
        let path = PathBuf::from(path);

        if !path.is_file() {
            return Err(Error::NonMarkdownFile(path));
        }

        let has_md_ext = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !has_md_ext {
            return Err(Error::NonMarkdownFile(path));
        }

        let input = fs::read_to_string(&path)?;

        let mut events = Transformer::new(Parser::new_ext(&input, options));
        let mut content = String::new();
        html::push_html(&mut content, &mut events);

        let html = create_page(
            content,
            vec![element!("title", |el| {
                if !events.meta().is_empty() {
                    el.after(events.meta(), ContentType::Html);
                }
                Ok(())
            })],
        )?;

        // look for `<meta name="date" content="" />` to extract the posts creation date
        let mut created_at = None;
        if !events.meta().is_empty() {
            rewrite_str(
                events.meta(),
                RewriteStrSettings {
                    element_content_handlers: vec![element!("meta[name=date]", |el| {
                        created_at = el.get_attribute("content").and_then(|content| {
                            DateTime::parse_from_rfc3339(&content)
                                .or_else(|_| DateTime::parse_from_rfc2822(&content))
                                .map(|dt| DateTime::<Utc>::from_utc(dt.naive_utc(), Utc))
                                .ok()
                        });
                        Ok(())
                    })],
                    ..RewriteStrSettings::default()
                },
            )?;
        }

        let mut out_path = PathBuf::new();
        out_path.push(out_dir);
        let file_name = path.with_extension("html");
        let mut file_name = file_name.file_name().unwrap().to_string_lossy().to_string();

        if let Ok(date) = NaiveDate::parse_from_str(&file_name[..10], "%F") {
            // remove date from filename; remove one more character that separates the date from
            // the slug (don't care about whether it is a _, -, space, or something else)
            file_name.replace_range(..11, "");

            if created_at.is_none() {
                created_at = Some(DateTime::<Utc>::from_utc(date.and_hms(0, 0, 0), Utc));
            }
        }

        let created_at = created_at.ok_or_else(|| Error::MissingDate(path.clone()))?;

        out_path.push(&*file_name);
        fs::write(out_path, html)?;

        posts.push(Post {
            file_name,
            title: events.title().map(String::from).unwrap_or_default(),
            created_at,
        });
    }

    create_index(posts)?;

    Ok(())
}

fn create_page(
    content: String,
    mut element_content_handlers: Vec<(Cow<'_, Selector>, ElementContentHandlers<'_>)>,
) -> Result<String, Error> {
    element_content_handlers.extend(vec![
        element!("[role=main]", move |el| {
            el.set_inner_content(&content, ContentType::Html);
            Ok(())
        }),
        element!("title", |el| {
            el.set_inner_content(CONFIG.site_name(), ContentType::Text);
            Ok(())
        }),
        element!("#header h1 a", |el| {
            el.set_inner_content(CONFIG.site_name(), ContentType::Html);
            Ok(())
        }),
        element!("#link-github", |el| {
            if let Some(username) = CONFIG.github_handle() {
                let _ = el.set_attribute("href", &format!("https://github.com/{}", username));
            } else {
                el.remove();
            }
            Ok(())
        }),
        element!("#link-twitter", |el| {
            if let Some(handle) = CONFIG.twitter_handle() {
                let _ = el.set_attribute("href", &format!("https://twitter.com/{}", handle));
            } else {
                el.remove();
            }
            Ok(())
        }),
    ]);

    Ok(rewrite_str(
        include_str!("theme/layout.html"),
        RewriteStrSettings {
            element_content_handlers,
            ..RewriteStrSettings::default()
        },
    )?)
}

fn create_index(mut posts: Vec<Post>) -> Result<(), Error> {
    posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let mut html = r#"<ul class="posts">"#.to_string();
    for post in posts {
        html += "<li>";
        html += &rewrite_str(
            include_str!("theme/post.html"),
            RewriteStrSettings {
                element_content_handlers: vec![
                    element!("a.post-link", |el| {
                        let _ = el.set_attribute("href", &post.file_name);
                        el.set_inner_content(&post.title, ContentType::Text);
                        Ok(())
                    }),
                    element!("time", |el| {
                        let _ = el.set_attribute("datetime", &post.created_at.to_rfc3339());
                        el.set_inner_content(
                            &post.created_at.date().format("%F").to_string(),
                            ContentType::Text,
                        );
                        Ok(())
                    }),
                ],
                ..RewriteStrSettings::default()
            },
        )?;
        html += "</li>";
    }
    html += "</ul>";

    let html = create_page(html, vec![])?;

    let mut out_path = PathBuf::new();
    out_path.push(CONFIG.out_dir());
    out_path.push("index.html");
    fs::write(out_path, html)?;

    Ok(())
}

#[derive(Debug)]
struct Post {
    file_name: String,
    title: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to rewrite layout.html")]
    Html(#[from] lol_html::errors::RewritingError),
    #[error("path is not a markdown file: {0}")]
    NonMarkdownFile(PathBuf),
    #[error("could not extract date for post: {0}")]
    MissingDate(PathBuf),
}
