mod config;
mod feed;
mod transform;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::config::CONFIG;
use crate::transform::Transformer;
use chrono::{DateTime, NaiveDate, Utc};
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, ElementContentHandlers, RewriteStrSettings, Selector};
use pulldown_cmark::{html, Options, Parser};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Error> {
    pretty_env_logger::formatted_builder()
        .filter_module("mdr", log::LevelFilter::Debug)
        .init();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let out_dir = CONFIG.out_dir();
    fs::create_dir_all(out_dir)?;

    // prepare layout
    let element_content_handlers = vec![
        element!("link[rel=stylesheet]", |el| {
            if let Some(href) = el.get_attribute("href") {
                match href.as_str() {
                    "normalize.css" => {
                        let new_href = hash_and_write(
                            "normalize",
                            Some("css"),
                            include_str!("theme/normalize.css"),
                        )?;
                        el.set_attribute("href", &new_href)?;
                    }
                    "style.css" => {
                        let new_href =
                            hash_and_write("style", Some("css"), include_str!("theme/style.css"))?;
                        el.set_attribute("href", &new_href)?;
                    }
                    _ => {}
                }
            }

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
    ];
    let layout = rewrite_str(
        include_str!("theme/layout.html"),
        RewriteStrSettings {
            element_content_handlers,
            ..RewriteStrSettings::default()
        },
    )?;

    // write posts
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

        // Collect assets from post.
        let content = rewrite_str(
            &content,
            RewriteStrSettings {
                element_content_handlers: vec![element!("img", |el| {
                    if let Some(src) = el.get_attribute("src") {
                        let src = Path::new(&src);
                        if src.is_absolute() {
                            return Ok(());
                        }

                        // relative to post's markdown file
                        let src = if let Some(base) = path.parent() {
                            base.join(src)
                        } else {
                            src.to_path_buf()
                        };
                        if !src.is_file() {
                            return Ok(());
                        }

                        let name = src.file_stem().and_then(|n| n.to_str()).unwrap_or("image");
                        let ext = src.extension().and_then(|n| n.to_str());
                        let new_src = hash_and_write(name, ext, &fs::read(&src)?)?;
                        el.set_attribute("src", &new_src)?;

                        // TODO: wrap in link
                        // TODO: convert image type?
                    }
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )?;

        let html = create_page(
            &layout,
            &content,
            vec![element!("title", |el| {
                if !events.meta().is_empty() {
                    el.after(events.meta(), ContentType::Html);
                }

                if let Some(title) = events.title() {
                    el.set_inner_content(
                        &format!("{} - {}", title, CONFIG.site_name()),
                        ContentType::Text,
                    );
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
            content,
            created_at,
        });
    }

    posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    create_index(&layout, &posts)?;
    feed::create(&posts)?;

    Ok(())
}

fn create_page(
    layout: &str,
    content: &str,
    handlers: Vec<(Cow<'_, Selector>, ElementContentHandlers<'_>)>,
) -> Result<String, Error> {
    let mut element_content_handlers = vec![element!("[role=main]", move |el| {
        el.set_inner_content(content, ContentType::Html);
        Ok(())
    })];
    element_content_handlers.extend(handlers);

    Ok(rewrite_str(
        layout,
        RewriteStrSettings {
            element_content_handlers,
            ..RewriteStrSettings::default()
        },
    )?)
}

fn create_index(layout: &str, posts: &[Post]) -> Result<(), Error> {
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

    let html = create_page(
        layout,
        &html,
        vec![element!("head", |el| {
            if CONFIG.url().is_some() {
                el.append(
                    "<link href=\"/feed.xml\" type=\"application/atom+xml\" \
                        rel=\"alternate\" title=\"Atom feed\" />\n",
                    ContentType::Html,
                );
            }

            Ok(())
        })],
    )?;

    let mut out_path = PathBuf::new();
    out_path.push(CONFIG.out_dir());
    out_path.push("index.html");
    fs::write(out_path, html)?;

    Ok(())
}

fn hash_and_write(name: &str, ext: Option<&str>, content: impl AsRef<[u8]>) -> io::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(content.as_ref());
    let hash = hasher.finalize();

    let hashed_name = if let Some(ext) = ext {
        format!(
            "{}-{}.{}",
            name,
            base64::encode_config(&hash[..16], base64::URL_SAFE_NO_PAD),
            ext
        )
    } else {
        format!(
            "{}-{}",
            name,
            base64::encode_config(&hash[..16], base64::URL_SAFE_NO_PAD)
        )
    };
    let mut path = PathBuf::new();
    path.push(CONFIG.out_dir());
    path.push(&hashed_name);
    fs::write(path, content)?;
    Ok(hashed_name)
}

#[derive(Debug)]
pub struct Post {
    file_name: String,
    title: String,
    content: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to rewrite layout.html")]
    Html(#[from] lol_html::errors::RewritingError),
    #[error("path is not a markdown file: {0}")]
    NonMarkdownFile(PathBuf),
    #[error("could not extract date for post: {0}")]
    MissingDate(PathBuf),
    #[error("failed to write feed.xml")]
    Xml(#[from] xml::writer::Error),
}
