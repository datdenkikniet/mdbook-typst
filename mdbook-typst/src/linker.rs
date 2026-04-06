use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use anyhow::Context;

fn div_wrap(text: &str) -> String {
    format!(r#"<div class="typst">{text}</div>"#)
}

pub struct Linker {
    inner: LinkerInner,
}

impl Linker {
    pub fn new_inline() -> Self {
        Self {
            inner: LinkerInner::Inline,
        }
    }

    pub fn new_in_source(book_source: &Path, generated_subdir: &Path) -> anyhow::Result<Self> {
        assert!(generated_subdir.is_relative());

        if let Ok(entries) = std::fs::read_dir(book_source.join(generated_subdir)) {
            for entry in entries {
                let entry = entry.context("failed to read entry from generated dir")?;
                std::fs::remove_file(entry.path())
                    .with_context(|| format!("failed to delete file {}", entry.path().display()))?;
            }
        }

        Ok(Self {
            inner: LinkerInner::Source {
                book_source: book_source.to_path_buf(),
                generated_subdir: generated_subdir.to_path_buf(),
            },
        })
    }

    pub fn replace(
        &self,
        source_path: &Path,
        source_content: &str,
        svg: String,
    ) -> anyhow::Result<String> {
        self.inner.replace(source_path, source_content, &svg)
    }
}

fn svg_name(path: &Path, content: &str) -> String {
    let mut path_hash = std::hash::DefaultHasher::new();
    path.hash(&mut path_hash);
    let path_hash = path_hash.finish();

    let mut content_hash = std::hash::DefaultHasher::new();
    content.hash(&mut content_hash);
    let content_hash = content_hash.finish();

    format!("{path_hash:016X}_{content_hash:016X}.svg")
}

enum LinkerInner {
    Inline,
    Source {
        book_source: PathBuf,
        generated_subdir: PathBuf,
    },
}

impl LinkerInner {
    fn replace(&self, source: &Path, source_content: &str, svg: &str) -> anyhow::Result<String> {
        match self {
            LinkerInner::Inline => Ok(div_wrap(svg)),
            LinkerInner::Source {
                book_source,
                generated_subdir,
            } => {
                let svg_name = svg_name(&source, &source_content);
                let output = book_source.join(generated_subdir).join(&svg_name);

                let directory = output.parent().unwrap();

                std::fs::create_dir_all(&directory)
                    .context("failed to create image destination dir")?;

                std::fs::write(&output, &svg).with_context(|| {
                    format!(
                        "failed to write SVG generated for page '{}' to '{}'",
                        source.display(),
                        output.display()
                    )
                })?;

                let mut svg_path: PathBuf = source.components().skip(1).map(|_| "..").collect();
                svg_path.push(generated_subdir);
                svg_path.push(svg_name);

                let link = format!("\n\n![](./{})\n\n", svg_path.display());

                Ok(div_wrap(&link))
            }
        }
    }
}
