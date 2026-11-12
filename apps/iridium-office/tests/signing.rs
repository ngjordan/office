//! The SignPath artifact configuration and the binary's VERSIONINFO have to agree.
//!
//! SignPath checks the metadata restrictions in the artifact configuration against the
//! executable's VERSIONINFO, and rejects the signing request when they differ. The values
//! live in two files that are edited for different reasons, so they drift: someone renames
//! the product in `build.rs` and every release from then on fails to sign, silently, at the
//! last step.
//!
//! These assertions are the drift check. They parse the two files with string matching rather
//! than pulling in an XML parser: both files are small, stable and hand-written.

use std::fs;
use std::path::PathBuf;

/// The executable's name, as the packaging script and the artifact configuration both spell it.
const EXE: &str = "iridium-office.exe";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn build_rs() -> String {
    fs::read_to_string(root().join("build.rs")).expect("build.rs")
}

fn artifact_configuration() -> String {
    fs::read_to_string(root().join("../../packaging/windows/signpath/office-suite.xml"))
        .expect("the SignPath artifact configuration")
}

/// The value of a `.set("KEY", "VALUE")` call in build.rs.
///
/// Tolerant of the line break rustfmt puts in a long call, so reformatting build.rs does not
/// break this check.
fn versioninfo(build_rs: &str, key: &str) -> String {
    let needle = format!("\"{key}\"");
    let start = build_rs
        .find(&needle)
        .unwrap_or_else(|| panic!("build.rs does not set {key}"))
        + needle.len();
    let after = &build_rs[start..];
    let comma = after.find(',').expect("a .set() call with one argument");
    let rest = &after[comma + 1..];
    let open = rest.find('"').expect("a quoted value in the .set() call");
    let rest = &rest[open + 1..];
    let end = rest.find('"').expect("unterminated string in build.rs");
    rest[..end].to_string()
}

/// The value of `attribute="value"` in the artifact configuration.
fn attribute(xml: &str, attribute: &str) -> String {
    let needle = format!("{attribute}=\"");
    let start = xml
        .find(&needle)
        .unwrap_or_else(|| panic!("the artifact configuration does not set {attribute}"))
        + needle.len();
    let rest = &xml[start..];
    let end = rest
        .find('"')
        .expect("unterminated string in the artifact configuration");
    rest[..end].to_string()
}

#[test]
fn the_signing_configuration_and_the_binary_metadata_agree() {
    let build_rs = build_rs();
    let xml = artifact_configuration();

    // SignPath checks these on every signing request, so a change in one file has to be
    // made in the other.
    assert_eq!(
        versioninfo(&build_rs, "ProductName"),
        attribute(&xml, "product-name")
    );
    assert_eq!(
        versioninfo(&build_rs, "CompanyName"),
        attribute(&xml, "company-name")
    );
    // The configuration uses the dynamic form: the file name of the executable it signs.
    assert_eq!(attribute(&xml, "original-filename"), "${file.name}");
    assert_eq!(versioninfo(&build_rs, "OriginalFilename"), EXE);

    // The copyright travels with the signature.
    assert_eq!(
        versioninfo(&build_rs, "LegalCopyright"),
        attribute(&xml, "copyright")
    );
}

#[test]
fn the_executable_name_is_the_same_everywhere() {
    let xml = artifact_configuration();
    let package_ps1 = fs::read_to_string(root().join("../../packaging/windows/package.ps1"))
        .expect("package.ps1");

    // The executable the configuration signs has to be the one the build produces.
    assert!(
        xml.contains(&format!("path=\"{EXE}\"")),
        "the configuration does not sign {EXE}"
    );
    // Inside the installer the executable sits in Program Files\Iridium\Office, so the
    // path in the configuration has to name that directory: a path that matches no file
    // fails the signing request.
    assert!(
        xml.contains("PFiles64/Iridium/Office/iridium-office.exe"),
        "the configuration does not find the executable inside the installer"
    );
    let wxs = fs::read_to_string(root().join("../../packaging/windows/iridium-office.wxs"))
        .expect("the installer source");
    assert!(
        wxs.contains("ProgramFiles6432Folder") && wxs.contains("Name=\"Iridium\""),
        "the installer no longer puts the suite in Program Files/Iridium"
    );
    assert!(
        package_ps1.contains(EXE),
        "the packaging script does not ship {EXE}"
    );

    // The installer is matched by a glob, so only the stem is fixed: the version and the
    // architecture sit in the middle of the name.
    assert!(
        xml.contains("iridium-office-*-windows-*.msi"),
        "the configuration expects a different MSI name"
    );
    assert!(
        package_ps1.contains("iridium-office-$Version-windows-$Arch.msi"),
        "the packaging script builds a differently named MSI"
    );
}

#[test]
fn the_version_in_the_binary_comes_from_the_crate() {
    let build_rs = build_rs();
    // The signing configuration compares the version against the four-part form that the
    // VERSIONINFO fields encode, so the binary has to take it from the crate version and
    // not from a hard-coded number.
    assert!(
        build_rs.contains("env!(\"CARGO_PKG_VERSION\")"),
        "build.rs does not take the version from the crate"
    );
    // A release run can override the version. The binary has to carry it, or the
    // version the signing service compares against is not the version in the binary.
    assert!(
        build_rs.contains("IRIDIUM_OFFICE_VERSION"),
        "build.rs does not take the version override"
    );
    assert!(
        build_rs.contains("cargo:rerun-if-env-changed=IRIDIUM_OFFICE_VERSION"),
        "build.rs does not rebuild when the version override changes"
    );
    // The numeric fields are the packed form; the string fields carry the same four-part
    // version, because the signing service compares the version against the artifact
    // configuration and may read either one.
    assert!(
        build_rs.contains(".set(\"FileVersion\""),
        "build.rs does not set the string version fields"
    );
    assert!(
        build_rs.contains(".set(\"ProductVersion\""),
        "build.rs does not set the string version fields"
    );
    // The string version is built from the crate version, not written out by hand.
    assert!(
        build_rs.contains("format!(\"{major}.{minor}.{patch}.{release}\")"),
        "the string version is not derived from the crate version"
    );
}

#[test]
fn each_windows_architecture_reports_its_own_signing_state() {
    let workflow = fs::read_to_string(root().join("../../.github/workflows/release.yml"))
        .expect("release.yml");

    // The Windows job runs once per architecture and both wrote the signing marker to the
    // same path. The publish job merges the two artifacts, so one architecture's marker
    // overwrote the other's, and the release notes described one architecture for both.
    assert!(
        !workflow.contains("dist\\release\\SIGNING'"),
        "the signing marker is still written to a shared path"
    );
    assert!(
        workflow.contains("SIGNING-${{ matrix.arch }}"),
        "the signing marker is not named per architecture"
    );
    // The marker is not a download: it is read for the release notes.
    assert!(
        !workflow.contains("! -name 'SIGNING' |"),
        "the shared marker is still excluded by an exact name"
    );
    assert!(
        workflow.contains("! -name 'SIGNING-*'"),
        "the per-architecture markers are not excluded from the downloads"
    );
    // Every architecture has to be signed for the release to say the artifacts are signed.
    assert!(
        workflow.contains("all_signed"),
        "the release notes do not require every architecture to be signed"
    );
}
