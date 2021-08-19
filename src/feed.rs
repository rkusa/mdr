use std::fs::File;
use std::path::PathBuf;

use super::{Error, Post};
use crate::config::CONFIG;
use xml::writer::events::XmlEvent;
use xml::writer::EventWriter;
use xml::EmitterConfig;

pub fn create(posts: &[Post]) -> Result<(), Error> {
    let url = match CONFIG.url() {
        Some(url) => {
            if url.ends_with('/') && !url.is_empty() {
                &url[..url.len() - 1]
            } else {
                url
            }
        }
        // only generate feed.xml if the url to the site is set
        None => return Ok(()),
    };

    let mut out_path = PathBuf::new();
    out_path.push(CONFIG.out_dir());
    out_path.push("feed.xml");

    let mut wr = EventWriter::new_with_config(
        File::create(out_path)?,
        EmitterConfig {
            perform_indent: true,
            ..EmitterConfig::default()
        },
    );
    wr.write(XmlEvent::StartDocument {
        version: xml::common::XmlVersion::Version10,
        encoding: Some("utf-8"),
        standalone: None,
    })?;
    wr.write(XmlEvent::start_element("feed").default_ns("http://www.w3.org/2005/Atom"))?;

    // title
    wr.write(XmlEvent::start_element("title"))?;
    wr.write(XmlEvent::characters(CONFIG.site_name()))?;
    wr.write(XmlEvent::end_element())?;

    // link to feed
    let feed_url = format!("{}/feed.xml", url);
    wr.write(
        XmlEvent::start_element("link")
            .attr("href", &feed_url)
            .attr("rel", "self"),
    )?;
    wr.write(XmlEvent::end_element())?;

    // link to site
    wr.write(XmlEvent::start_element("link").attr("href", url))?;
    wr.write(XmlEvent::end_element())?;

    // id
    let id = format!("{}/", url);
    wr.write(XmlEvent::start_element("id"))?;
    wr.write(XmlEvent::characters(&id))?;
    wr.write(XmlEvent::end_element())?;

    // updated
    if let Some(post) = posts.first() {
        wr.write(XmlEvent::start_element("updated"))?;
        wr.write(XmlEvent::characters(&post.created_at.to_rfc3339()))?;
        wr.write(XmlEvent::end_element())?;
    }

    // author
    wr.write(XmlEvent::start_element("author"))?;
    wr.write(XmlEvent::start_element("name"))?;
    wr.write(XmlEvent::characters(CONFIG.site_name()))?;
    wr.write(XmlEvent::end_element())?;
    wr.write(XmlEvent::end_element())?;

    for post in posts {
        wr.write(XmlEvent::start_element("entry"))?;

        // title
        wr.write(XmlEvent::start_element("title"))?;
        wr.write(XmlEvent::characters(&post.title))?;
        wr.write(XmlEvent::end_element())?;

        // link
        let post_url = format!("{}/{}", url, post.file_name);
        wr.write(XmlEvent::start_element("link").attr("href", &post_url))?;
        wr.write(XmlEvent::end_element())?;

        // id
        wr.write(XmlEvent::start_element("id"))?;
        wr.write(XmlEvent::characters(&post_url))?;
        wr.write(XmlEvent::end_element())?;

        // updated
        wr.write(XmlEvent::start_element("updated"))?;
        wr.write(XmlEvent::characters(&post.created_at.to_rfc3339()))?;
        wr.write(XmlEvent::end_element())?;

        // content
        wr.write(XmlEvent::start_element("content").attr("type", "html"))?;
        wr.write(XmlEvent::CData(&post.content))?;
        wr.write(XmlEvent::end_element())?;

        wr.write(XmlEvent::end_element())?;
    }

    wr.write(XmlEvent::end_element())?;

    Ok(())
}

// <feed xmlns="http://www.w3.org/2005/Atom">

// 	<title>Example Feed</title>
// 	<subtitle>A subtitle.</subtitle>
// 	<link href="http://example.org/feed/" rel="self" />
// 	<link href="http://example.org/" />
// 	<id>urn:uuid:60a76c80-d399-11d9-b91C-0003939e0af6</id>
// 	<updated>2003-12-13T18:30:02Z</updated>

// 	<entry>
// 		<title>Atom-Powered Robots Run Amok</title>
// 		<link href="http://example.org/2003/12/13/atom03" />
// 		<link rel="alternate" type="text/html" href="http://example.org/2003/12/13/atom03.html"/>
// 		<link rel="edit" href="http://example.org/2003/12/13/atom03/edit"/>
// 		<id>urn:uuid:1225c695-cfb8-4ebb-aaaa-80da344efa6a</id>
// 		<updated>2003-12-13T18:30:02Z</updated>
// 		<summary>Some text.</summary>
// 		<content type="xhtml">
// 			<div xmlns="http://www.w3.org/1999/xhtml">
// 				<p>This is the entry content.</p>
// 			</div>
// 		</content>
// 		<author>
// 			<name>John Doe</name>
// 			<email>johndoe@example.com</email>
// 		</author>
// 	</entry>

// </feed>
