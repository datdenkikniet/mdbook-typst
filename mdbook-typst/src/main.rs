use std::{
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use mdbook_preprocessor::book::BookItem;

use ::typst::{ecow::EcoString, syntax::package::PackageSpec};
use anyhow::{Context, Result};
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};
use serde::Deserialize;

use crate::{download::AutoDownload, typst::MdbookWorld};

mod download;
mod typst;

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "toml_false", rename = "auto-download")]
    pub auto_download: toml::Value,
    #[serde(default = "typst_pkgs", rename = "package-root")]
    pub package_root: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_download: toml_false(),
            package_root: typst_pkgs(),
        }
    }
}

fn toml_false() -> toml::Value {
    toml::Value::Boolean(false)
}

fn typst_pkgs() -> PathBuf {
    "typst-pkgs".into()
}

/// If no command is provided, this tool parses mdbook preprocessor input from stdin
/// and writes the transformed result to stdout.
#[derive(clap::Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Parser)]
enum Command {
    /// Validate whether this preprocessor supports an mdbook backend.
    Supports { backend: String },

    /// Download a package from the typst universe.
    ///
    /// This command requires that 'wget' and 'gunzip' are
    /// available as commands.
    ///
    /// This command also validates that the downloaded package
    /// has an allowed license.
    Download {
        /// The name of the package to download from the
        /// typst universe, in @namespace/package:version format.
        package: String,
        /// The destination folder. Usually `book/typst-pkgs`.
        dest: PathBuf,
        /// Licenses that are accepted.
        #[clap(default_value = "MIT", value_delimiter = ',', short, long, env)]
        allowed_licenses: Vec<String>,
    },
}

fn main() -> Result<()> {
    use clap::Parser;
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Download {
            package,
            dest,
            allowed_licenses,
        }) => {
            let spec: PackageSpec = FromStr::from_str(&package)
                .map_err(|e: EcoString| anyhow::format_err!(e))
                .context("Failed to parse package spec")?;

            AutoDownload::BuiltIn
                .download_if_absent(&dest, &spec, &allowed_licenses)
                .map(|_| ())
                .map_err(Into::into)
        }
        Some(Command::Supports { backend }) => {
            if backend == "html" {
                Ok(())
            } else {
                Err(anyhow::anyhow!("{backend} backend is not supported."))
            }
        }
        None => {
            let output = book()?;
            std::io::stdout()
                .write_all(output.as_bytes())
                .map_err(Into::into)
        }
    }
}

fn book() -> Result<String> {
    let (ctx, mut book) = mdbook_preprocessor::parse_input(std::io::stdin())?;

    let config: Config = ctx.config.get("preprocessor.typst")?.unwrap_or_default();

    let pkg_root = ctx.root.join(config.package_root);
    let book_source = ctx.root.join(&ctx.config.book.src);

    let auto_download = match config.auto_download {
        toml::Value::String(v) => Some(AutoDownload::Custom(v)),
        toml::Value::Boolean(true) => Some(AutoDownload::BuiltIn),
        toml::Value::Boolean(false) => None,
        v => anyhow::bail!(
            "Unexpected value '{v}' for preprocessor.typst.autodownload, expected String or Boolean"
        ),
    };

    let world = MdbookWorld::new(
        pkg_root,
        auto_download,
        vec!["MIT".into(), "MIT OR Apache-2.0".into()],
        book_source,
    );
    replace_typst(&world, &mut book.items)?;
    serde_json::to_string(&book).context("Failed to serialize book")
}

fn div_wrap(text: &str) -> String {
    format!(r#"<div class="typst">{text}</div>"#)
}

fn replace_typst<'a>(world: &MdbookWorld, items: &mut [BookItem]) -> Result<()> {
    let chapters = items.iter_mut().filter_map(|v| {
        if let BookItem::Chapter(c) = v {
            Some(c)
        } else {
            None
        }
    });

    for chapter in chapters {
        let Some(path) = chapter.path.as_ref() else {
            continue;
        };

        let content = &chapter.content;
        let mut parser = Parser::new(&content).into_offset_iter();

        let mut replacements = Vec::new();
        while let Some((event, range)) = parser.next() {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                    if info.as_ref() != "typst" {
                        continue;
                    }

                    let (event, text_range) = parser.next().unwrap();

                    if event != Event::End(TagEnd::CodeBlock) {
                        assert_eq!(parser.next().unwrap().0, Event::End(TagEnd::CodeBlock));
                    }

                    let text = &content[text_range];
                    let result = world.compile(path, text.to_string());

                    replacements.push((range, div_wrap(&result)));
                }

                Event::Start(Tag::Link {
                    link_type: LinkType::Autolink,
                    dest_url,
                    ..
                }) => {
                    let Some(file) = dest_url.strip_prefix("typst://") else {
                        continue;
                    };

                    let file = Path::new(file);
                    let full_path = world.book_source.join(file);

                    let text = match std::fs::read_to_string(full_path) {
                        Ok(v) => v,
                        Err(e) => {
                            if e.kind() == ErrorKind::NotFound {
                                anyhow::bail!(
                                    "Could not find file '{}' in book source.",
                                    file.display()
                                );
                            } else if e.kind() == ErrorKind::InvalidData {
                                anyhow::bail!(
                                    "File to be included '{}' contains invalid UTF-8",
                                    file.display()
                                );
                            } else {
                                return Err(e.into());
                            }
                        }
                    };

                    let result = world.compile(file, text);
                    replacements.push((range, div_wrap(&result)));
                }
                _ => {}
            }
        }

        let mut output = String::new();
        let mut last_index = 0;

        for (range, replacement) in replacements {
            output.push_str(&content[last_index..range.start]);
            last_index = range.end;
            output.push_str(&replacement);
        }

        output.push_str(&content[last_index..]);

        chapter.content = output;

        replace_typst(world, &mut chapter.sub_items)?;
    }

    Ok(())
}
