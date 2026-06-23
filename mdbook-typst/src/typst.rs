use std::{
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use normalize_path::NormalizePath;
use typst::{
    Library, LibraryExt,
    diag::{FileError, FileResult, PackageError},
    foundations::{Bytes, Datetime, Duration},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot, package::PackageSpec},
    text::{Font, FontBook, FontInfo},
    utils::LazyHash,
};
use typst_layout::PagedDocument;
use typst_svg::SvgOptions;

use crate::download::AutoDownload;

pub fn rel_path(package: &PackageSpec) -> String {
    format!("{}/{}-{}", package.namespace, package.name, package.version)
}

pub struct MdbookWorld {
    pkg_root: PathBuf,
    pub book_source: PathBuf,
    auto_download: Option<AutoDownload>,
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: Vec<(Font, FontInfo)>,
    allowed_licenses: Vec<String>,
}

impl MdbookWorld {
    pub fn new(
        pkg_root: PathBuf,
        auto_download: Option<AutoDownload>,
        allowed_licenses: Vec<String>,
        book_source: PathBuf,
    ) -> Self {
        let fonts: Vec<_> = typst_kit::fonts::embedded().collect();
        let font_book = FontBook::from_fonts(fonts.iter().map(|(f, _)| f));

        let done = Self {
            library: LazyHash::new(Library::default()),
            fonts,
            font_book: LazyHash::new(font_book),
            pkg_root,
            book_source,
            auto_download,
            allowed_licenses,
        };

        done
    }

    pub fn compile(&self, source_path: &Path, source_contents: &str) -> String {
        let source = source_path.to_string_lossy();
        let source_path = FileId::new(RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new(source).unwrap(),
        ));

        let world = BookWorldInner {
            pkg_root: &self.pkg_root,
            library: &self.library,
            fonts: &self.fonts,
            font_book: &self.font_book,
            book_source: &self.book_source,
            auto_download: self.auto_download.as_ref(),
            allowed_licenses: &self.allowed_licenses,
            source_path,
            source_contents,
        };

        let output = typst::compile::<PagedDocument>(&world);

        let pages = output.output.unwrap();
        let first_page = pages.pages().first().unwrap();
        let done = typst_svg::svg(first_page, &SvgOptions::default());

        done
    }
}

struct BookWorldInner<'a> {
    library: &'a LazyHash<Library>,
    fonts: &'a Vec<(Font, FontInfo)>,
    font_book: &'a LazyHash<FontBook>,
    pkg_root: &'a Path,
    book_source: &'a Path,
    source_path: FileId,
    source_contents: &'a str,
    auto_download: Option<&'a AutoDownload>,
    allowed_licenses: &'a [String],
}

enum File {
    Source(String),
    Data(Vec<u8>),
}

impl BookWorldInner<'_> {
    fn find(&self, id: FileId) -> FileResult<File> {
        if let VirtualRoot::Package(package) = id.root() {
            let tar = self
                .pkg_root
                .join(rel_path(package))
                .with_added_extension("tar");

            if let Some(auto_download) = self.auto_download {
                auto_download.download_if_absent(&tar, &package, &self.allowed_licenses)?;
            }

            find_in_tar(&tar, package, id.vpath()).map(File::Data)
        } else if self.source_path == id {
            Ok(File::Source(self.source_contents.to_string()))
        } else {
            // A rootless path is always relative to the
            // compilation source
            let file_path = id.vpath().get_without_slash();
            let path = self.book_source.join(file_path);

            // Normalize the path to ensure we're not trying
            // to read data outside of our source.
            let path = path.normalize();

            if !path.starts_with(self.book_source) {
                return Err(FileError::AccessDenied);
            }

            std::fs::read(&path)
                .map_err(|_| FileError::NotFound(path))
                .map(File::Data)
        }
    }
}

impl typst::World for BookWorldInner<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_book
    }

    fn main(&self) -> FileId {
        self.source_path
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.find(id)? {
            File::Source(s) => Ok(Source::new(id, s)),
            File::Data(data) => {
                let data = String::from_utf8(data).map_err(|_| FileError::NotSource)?;
                Ok(Source::new(id, data))
            }
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        match self.find(id)? {
            File::Source(source) => Ok(Bytes::new(source.into_bytes())),
            File::Data(data) => Ok(Bytes::new(data)),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).map(|(f, _)| f.clone())
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        unimplemented!()
    }
}

fn find_in_tar(tar: &Path, package: &PackageSpec, file_path: &VirtualPath) -> FileResult<Vec<u8>> {
    let file = std::fs::File::open(&tar).map_err(|e| {
        eprintln!(
            "Failed to open package {path}. Error: {e:?}",
            path = tar.display(),
        );

        FileError::Package(PackageError::NotFound(package.clone()))
    })?;

    let file = BufReader::new(file);

    let mut archive = tar::Archive::new(file);

    for entry in archive
        .entries_with_seek()
        .map_err(|_| FileError::Package(PackageError::MalformedArchive(None)))?
    {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();

        if path.as_os_str().as_encoded_bytes() == file_path.get_without_slash().as_bytes() {
            let mut output = Vec::new();
            entry.read_to_end(&mut output).unwrap();
            return Ok(output);
        }
    }

    Err(FileError::NotFound(tar.to_path_buf()))
}
