//! Route a file to the app that owns it.
//!
//! The extension lists come from each app's own packaging metadata, not from guesswork:
//! `packaging/linux/*.desktop` `MimeType=` lines, resolved through the system
//! shared-mime-info database (`/usr/share/mime/packages/freedesktop.org.xml`), plus each
//! app's own `*.mime.xml` glob patterns.
//!
//! Sources, per app:
//! - Sheets: `ph.sheets.desktop` + `ph.sheets.mime.xml` -> csv, ods, tsv, xlsb, xlsm, xlsx
//! - Docs:   `ph.docs.desktop` -> doc, docm, docx, dot, dotm, dotx, dtx, htm, html, ins,
//!   latex, ltx, markdown, md, mkd, odt, rtf, sty, tex
//! - Cad:    `ph.cad.desktop` + `ph.cad.mime.xml` -> dwg, dxf
//! - Decks:  `ph.decks.desktop` + `ph.decks.mime.xml` -> iridium-decks, potx, ppsx, pptx
//! - PDF:    `ph.pdf.desktop` -> pdf
//! - Mail:   `ph.mail.desktop` -> eml, mbox, ics, vcf (and the Outlook formats msg, pst)
//! - Photos: `ph.photos.desktop` -> the raster and painting formats (see PHOTOS)
//! - Film:   `ph.film.desktop` -> the video and audio formats (see FILM)
//! - Light:  `ph.light.desktop` -> the camera RAW formats and the presets (see LIGHT)
//!
//! **Photos and Light both open ordinary photographs.** One app must own each
//! extension, so the two are split by what they are FOR: Photos is the raster
//! editor (Photoshop's job) and Light is the camera workflow (Lightroom's job).
//! A plain `jpg` is a picture to edit, so Photos owns it; a `cr2` is a camera
//! negative, so Light owns it. Light still opens plain photographs from inside
//! its own library, which is where that workflow belongs.

use crate::module::Kind;

/// The extensions each app registers, lowercase, without the dot.
const SHEETS: &[&str] = &["csv", "ods", "tsv", "xlsb", "xlsm", "xlsx"];
const DOCS: &[&str] = &[
    "doc", "docm", "docx", "dot", "dotm", "dotx", "dtx", "htm", "html", "ins", "latex", "ltx",
    "markdown", "md", "mkd", "odt", "rtf", "sty", "tex",
];
const CAD: &[&str] = &["dwg", "dxf"];
const DECKS: &[&str] = &["iridium-decks", "potx", "ppsx", "pptx"];
const PDF: &[&str] = &["pdf"];
const MAIL: &[&str] = &["eml", "mbox", "ics", "vcf", "msg", "pst"];

/// Photos: the raster and painting editor.
///
/// From the app's own source: `crates/io/src/lib.rs:499` (PSD_EXTS), the
/// `pcraft` project format, and the painting formats its `crates/io` decodes.
const PHOTOS: &[&str] = &[
    // The Photoshop family and the app's own project format.
    "pcraft", "psd", "psb", "psdt", // Painting and raster formats.
    "pdn", "kra", "krz", "pcx", "pbm", "pgm", "ppm", "pam", "xbm", "xpm", "sgi", "tga", "targa",
    "icb", "vda", "vst", "pix", "ani", "cur", "pcx2",
    // Ordinary pictures, which this app edits.
    "png", "jpg", "jpeg", "jfif", "jpe", "gif", "bmp", "tif", "tiff", "webp", "avif", "qoi", "exr",
    "hdr", "pic", "dib",
];

/// Film: the video editor.
///
/// From the app's own source: `crates/media/src/lib.rs:267-271` — STILL, AUDIO
/// and VIDEO_EXTENSIONS — plus the `fcproj` project format.
const FILM: &[&str] = &[
    // The project format.
    "fcproj", // Video.
    "mp4", "m4v", "mov", "mkv", "webm", "avi", "mxf", "mts", "m2ts", "m2t", "ts", "mpg", "mpeg",
    "vob", "mod", "tod", "m2v", "m1v", "mpv", "3gp", "y4m", "apv", // Audio.
    "wav", "wave", "bwf", "aif", "aiff", "mp3", "mp2", "m4a", "aac", "flac", "ogg", "opus",
    // Sequences: the editor imports stills as a timeline.
    "dpx", "r3d",
];

/// Light: the camera workflow.
///
/// From the app's own source: `crates/raw` style RAW handling and the presets
/// in `crates/ui-egui/src/pick.rs:61` (PRESET_EXTENSIONS).
const LIGHT: &[&str] = &[
    // Camera negatives.
    "dng",
    "cr2",
    "cr3",
    "nef",
    "nrw",
    "arw",
    "srf",
    "sr2",
    "pef",
    "orf",
    "rw2",
    "raf",
    "3fr",
    "fff",
    "raw",
    "erf",
    "mrw",
    "mos",
    "x3f",
    "dcr",
    "kdc",
    "srw",
    // HEIF, which is a camera format before it is a picture.
    "heic",
    "heif",
    // Presets, sidecars and the catalogue.
    "lcpreset",
    "xmp",
    "lrtemplate",
    "lccurve",
    "lmp",
    "mplumpack",
    "cube",
    "dcp",
    "lceffect",
];

/// Every extension any app registers, with its owner.
pub fn table() -> Vec<(&'static str, Kind)> {
    let mut v = Vec::new();
    for e in SHEETS {
        v.push((*e, Kind::Sheets));
    }
    for e in DOCS {
        v.push((*e, Kind::Docs));
    }
    for e in CAD {
        v.push((*e, Kind::Cad));
    }
    for e in DECKS {
        v.push((*e, Kind::Decks));
    }
    for e in PDF {
        v.push((*e, Kind::Pdf));
    }
    for e in MAIL {
        v.push((*e, Kind::Mail));
    }
    for e in PHOTOS {
        v.push((*e, Kind::Photos));
    }
    for e in FILM {
        v.push((*e, Kind::Film));
    }
    for e in LIGHT {
        v.push((*e, Kind::Light));
    }
    v
}

/// The app that owns a path, by its extension.
pub fn kind_for(path: &std::path::Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    table().into_iter().find(|(e, _)| *e == ext).map(|(_, k)| k)
}

/// The app that owns a string by its extension.
pub fn kind_for_str(name: &str) -> Option<Kind> {
    let ext = name.rsplit_once('.').map(|(_, e)| e)?;
    let ext = ext.to_ascii_lowercase();
    table().into_iter().find(|(e, _)| *e == ext).map(|(_, k)| k)
}

/// Split the command line's files by the app that owns them.
pub fn route(files: &[std::path::PathBuf]) -> Vec<(Kind, Vec<String>)> {
    let mut out: Vec<(Kind, Vec<String>)> = Vec::new();
    for f in files {
        let Some(k) = kind_for(f) else { continue };
        let s = f.to_string_lossy().into_owned();
        match out.iter_mut().find(|(k2, _)| *k2 == k) {
            Some((_, v)) => v.push(s),
            None => out.push((k, vec![s])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every extension any app registers maps to exactly one Kind.
    #[test]
    fn no_extension_is_registered_twice() {
        let t = table();
        let mut seen = std::collections::HashSet::new();
        for (ext, _) in &t {
            assert!(seen.insert(*ext), "extension {ext} is registered twice");
        }
    }

    #[test]
    fn every_app_registers_something() {
        for k in Kind::ALL {
            assert!(
                table().iter().any(|(_, k2)| *k2 == k),
                "{k:?} registers no extension"
            );
        }
    }

    #[test]
    fn routes_to_the_owning_app() {
        assert_eq!(kind_for_str("a.xlsx"), Some(Kind::Sheets));
        assert_eq!(kind_for_str("a.docx"), Some(Kind::Docs));
        assert_eq!(kind_for_str("a.dxf"), Some(Kind::Cad));
        assert_eq!(kind_for_str("a.pptx"), Some(Kind::Decks));
        assert_eq!(kind_for_str("a.pdf"), Some(Kind::Pdf));
        assert_eq!(kind_for_str("a.IRIDIUM-DECKS"), Some(Kind::Decks));
        assert_eq!(kind_for_str("a.eml"), Some(Kind::Mail));
        assert_eq!(kind_for_str("a.MBOX"), Some(Kind::Mail));
        assert_eq!(kind_for_str("a.pst"), Some(Kind::Mail));
    }

    #[test]
    fn unknown_extension_has_no_owner() {
        assert_eq!(kind_for_str("a.txt"), None);
        assert_eq!(kind_for_str("noextension"), None);
    }
}
