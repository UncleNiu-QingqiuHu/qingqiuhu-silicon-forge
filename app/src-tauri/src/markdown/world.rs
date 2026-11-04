//! Typst 编译环境的最小实现。
//!
//! Typst 需要一个 `World` 提供标准库、字体、源文件等。这里只支持单文件编译，
//! 不引入外部资源，符合「全程本地、不访问网络」的约束。
//!
//! 字体直接扫描系统字体目录：PDF 生成时字体会嵌入文件，
//! 因此客户机器缺字体也不影响查看效果。

use std::path::PathBuf;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

struct FontSet {
    book: FontBook,
    fonts: Vec<Font>,
}

static FONTS: OnceLock<FontSet> = OnceLock::new();

/// 扫描 Windows 字体目录。结果进程内缓存，只在首次导出 PDF 时付出一次开销。
fn system_fonts() -> &'static FontSet {
    FONTS.get_or_init(|| {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();

        let mut dirs: Vec<PathBuf> = vec![PathBuf::from(r"C:\Windows\Fonts")];
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join(r"Microsoft\Windows\Fonts"));
        }

        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                    continue;
                }
                let Ok(data) = std::fs::read(&path) else {
                    continue;
                };
                let bytes = Bytes::new(data);
                // .ttc 是字体集合，用递增索引逐个尝试，取到 None 为止
                for index in 0..24u32 {
                    let Some(font) = Font::new(bytes.clone(), index) else {
                        break;
                    };
                    book.push(font.info().clone());
                    fonts.push(font);
                }
            }
        }

        FontSet { book, fonts }
    })
}

pub struct ForgeWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    source: Source,
    /// Markdown 文件所在目录，用于解析相对路径的图片
    root: PathBuf,
}

impl ForgeWorld {
    pub fn new(text: String, root: PathBuf) -> Self {
        let set = system_fonts();
        // 固定字面量路径，解析失败属于代码错误而非运行期输入问题
        let vpath = VirtualPath::new("main.typ").expect("main.typ 不是合法的虚拟路径");
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();
        let source = Source::new(main, text);

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(set.book.clone()),
            fonts: set.fonts.clone(),
            main,
            source,
            root,
        }
    }
}

impl World for ForgeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from("main.typ")))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            return Ok(Bytes::from_string(self.source.text().to_string()));
        }

        // 只允许读取 root 目录内的资源，防止 Markdown 里的路径穿越到其他地方
        let rel = id.vpath().as_rootless_path();
        if rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(FileError::AccessDenied);
        }

        let full = self.root.join(rel);
        std::fs::read(&full)
            .map(Bytes::new)
            .map_err(|_| FileError::NotFound(full))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // 固定日期，保证同样输入产出同样 PDF，便于回归比对
        Datetime::from_ymd(2026, 1, 1)
    }
}
