//! Windows only: embed the app icon and version info (VERSIONINFO) into `iridium-office.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `IRIDIUM_OFFICE_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).
//!
//! The version fields come from the crate version, not from a hard-coded zero: Explorer shows
//! `0.0.0.0` otherwise, and the SignPath metadata restriction checks them.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // A release run overrides the version; the binary has to carry that version, because
    // the signing service compares the executable's version against the artifact
    // configuration, and the configuration is built from the same override.
    println!("cargo:rerun-if-env-changed=IRIDIUM_OFFICE_VERSION");
    println!("cargo:rerun-if-changed=../../assets/app-icon/iridium-office.ico");
    println!("cargo:rerun-if-env-changed=IRIDIUM_OFFICE_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // The version is the crate version, or the override a release run sets. Any
    // pre-release suffix is dropped, because the VERSIONINFO fields are four 16-bit
    // numbers.
    let version = std::env::var("IRIDIUM_OFFICE_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let mut parts = version
        .split(['-', '+'])
        .next()
        .unwrap_or("0.0.0")
        .split('.');
    let mut word = || {
        parts
            .next()
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let (major, minor, patch, release) = (word(), word(), word(), word());
    let version = (major << 48) | (minor << 32) | (patch << 16) | release;
    // The same version as the four-part string. winresource fills the string fields from
    // CARGO_PKG_VERSION ("0.2.0"), which is not the four-part form the numeric fields
    // encode; the signing service compares the version against the artifact
    // configuration, and which of the two it reads is not something to leave to chance.
    let version4 = format!("{major}.{minor}.{patch}.{release}");

    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/iridium-office.ico")
        .set("ProductName", "Office")
        .set("FileDescription", "Office suite")
        .set("CompanyName", "Iridium Technologies, Inc.")
        .set(
            "LegalCopyright",
            "Copyright (c) the Office authors. MIT OR Apache-2.0.",
        )
        .set("OriginalFilename", "iridium-office.exe")
        .set("InternalName", "iridium-office")
        .set("FileVersion", &version4)
        .set("ProductVersion", &version4);
    res.set_version_info(winresource::VersionInfo::FILEVERSION, version);
    res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, version);
    if let Err(e) = res.compile() {
        if std::env::var_os("IRIDIUM_OFFICE_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=iridium-office.exe built without icon/version resources: {e}");
    }
}
