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
    println!("cargo:rerun-if-changed=../../assets/app-icon/iridium-office.ico");
    println!("cargo:rerun-if-env-changed=IRIDIUM_OFFICE_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // The crate version is major.minor.patch; any pre-release suffix is dropped, because the
    // VERSIONINFO fields are four 16-bit numbers.
    let mut parts = env!("CARGO_PKG_VERSION")
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
        .set("InternalName", "iridium-office");
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
