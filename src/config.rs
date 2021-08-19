use clap::ArgMatches;
use clap::{App, Arg};
use once_cell::sync::Lazy;

pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    dotenv::dotenv().ok();

    let matches = App::new("mdr")
        .version(env!("CARGO_PKG_VERSION"))
        .about("simple opinionated markdown renderer")
        .arg(
            Arg::with_name("SITE_NAME")
                .long("name")
                .help("the site's name")
                .env("SITE_NAME")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("OUT_DIR")
                .long("out")
                .short("o")
                .help("the directory the build result is saved to")
                .env("OUT_DIR")
                .takes_value(true)
                .default_value("./out"),
        )
        .arg(
            Arg::with_name("TWITTER_HANDLE")
                .long("twitter")
                .help("your Twitter handle")
                .env("TWITTER_HANDLE")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("GITHUB_HANDLE")
                .long("github")
                .help("your Github handle")
                .env("GITHUB_HANDLE")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("URL")
                .long("url")
                .help("the absolute URL of your site")
                .env("URL")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("FILE")
                .help("the markdown files to render")
                .multiple(true)
                .required(true),
        )
        .get_matches();

    Config(matches)
});

pub struct Config(ArgMatches<'static>);

impl Config {
    pub fn site_name(&self) -> &str {
        self.0.value_of("SITE_NAME").unwrap_or("Blog")
    }

    pub fn out_dir(&self) -> &str {
        self.0.value_of("OUT_DIR").unwrap_or("./out")
    }

    pub fn twitter_handle(&self) -> Option<&str> {
        self.0.value_of("TWITTER_HANDLE")
    }

    pub fn github_handle(&self) -> Option<&str> {
        self.0.value_of("GITHUB_HANDLE")
    }

    pub fn url(&self) -> Option<&str> {
        self.0.value_of("URL")
    }

    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.0.values_of("FILE").unwrap()
    }
}
