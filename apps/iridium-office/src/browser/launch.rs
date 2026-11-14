//! Finding a browser and running it under our own profile.
//!
//! The rules, and why:
//!
//! - **Never the user's everyday browser.** Chrome 136+ refuses remote
//!   debugging on the default profile, and that profile holds their sessions.
//!   We write our own profile directory and point the browser at it.
//! - **Never `--no-sandbox`, `--disable-web-security`, or a wildcard origin
//!   allowance.** A debugging port is remote code execution on the machine;
//!   the sandbox is what keeps the page from having the browser's privileges.
//! - **Port 0, and read `DevToolsActivePort`.** The browser picks a free port
//!   itself and writes it out, so there is no race for the port number.
//! - **Kill only our own tree.** The child gets its own process group on Unix
//!   and a job object on Windows, so a crash cannot leave orphans behind.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The browsers we will drive, in order of preference.
///
/// Brave is first because it is Chromium and it already blocks trackers, which
/// is most of what "Brave-like" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Brave,
    Edge,
    Chrome,
    Chromium,
}

impl Browser {
    /// The name to show the user.
    pub fn label(self) -> &'static str {
        match self {
            Browser::Brave => "Brave",
            Browser::Edge => "Microsoft Edge",
            Browser::Chrome => "Google Chrome",
            Browser::Chromium => "Chromium",
        }
    }

    /// Where to look for this browser, in order.
    ///
    /// The first entry that exists wins. macOS app bundles are checked by
    /// their real executable path, not by `open`, so the child is ours to kill.
    fn candidates(self) -> Vec<PathBuf> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .unwrap_or_default();
        match self {
            Browser::Brave => vec![
                PathBuf::from("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
                PathBuf::from(
                    "/Applications/Brave Browser Beta.app/Contents/MacOS/Brave Browser Beta",
                ),
                PathBuf::from(
                    "/Applications/Brave Browser Nightly.app/Contents/MacOS/Brave Browser Nightly",
                ),
                home.join("Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
                PathBuf::from(
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                ),
                PathBuf::from("/usr/bin/brave-browser"),
                PathBuf::from("/usr/bin/brave"),
                PathBuf::from("/snap/bin/brave"),
            ],
            Browser::Edge => vec![
                PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
                home.join("Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
                PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
                PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
                PathBuf::from("/usr/bin/microsoft-edge"),
                PathBuf::from("/usr/bin/microsoft-edge-stable"),
            ],
            Browser::Chrome => vec![
                PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
                home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
                PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
                PathBuf::from("/usr/bin/google-chrome"),
                PathBuf::from("/usr/bin/google-chrome-stable"),
                PathBuf::from("/opt/google/chrome/chrome"),
            ],
            Browser::Chromium => vec![
                PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
                PathBuf::from(r"C:\Program Files\Chromium\Application\chrome.exe"),
                PathBuf::from("/usr/bin/chromium"),
                PathBuf::from("/usr/bin/chromium-browser"),
                PathBuf::from("/snap/bin/chromium"),
            ],
        }
    }
}

/// The browsers we try, best first.
const PREFERENCE: [Browser; 4] = [
    Browser::Brave,
    Browser::Edge,
    Browser::Chrome,
    Browser::Chromium,
];

/// Find an installed browser.
///
/// `IRIDIUM_OFFICE_BROWSER` overrides everything, for people whose browser is
/// somewhere unusual. The value is a path to the executable.
pub fn find_browser() -> Option<(Browser, PathBuf)> {
    if let Some(path) = std::env::var_os("IRIDIUM_OFFICE_BROWSER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some((browser_for(&path), path));
        }
    }
    for browser in PREFERENCE {
        for candidate in browser.candidates() {
            if candidate.is_file() {
                return Some((browser, candidate));
            }
        }
    }
    None
}

/// Guess which browser a path is, so the override still gets a label.
fn browser_for(path: &Path) -> Browser {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.contains("brave") {
        Browser::Brave
    } else if name.contains("edge") || name.contains("msedge") {
        Browser::Edge
    } else if name.contains("chrome") {
        Browser::Chrome
    } else {
        Browser::Chromium
    }
}

/// Why the browser could not be started.
#[derive(Debug)]
pub enum BrowserError {
    /// No supported browser is installed.
    NotFound,
    /// The browser was found but would not start.
    LaunchFailed(String),
    /// The debugging endpoint was not reachable.
    BadEndpoint(String),
    /// A DevTools Protocol call failed.
    Cdp(String),
    /// Something else went wrong.
    Other(String),
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserError::NotFound => write!(
                f,
                "no supported browser found. Install Brave, Edge, Chrome or Chromium, \
                 or set IRIDIUM_OFFICE_BROWSER to the browser executable"
            ),
            BrowserError::LaunchFailed(e) => write!(f, "the browser did not start: {e}"),
            BrowserError::BadEndpoint(e) => write!(f, "bad debugging endpoint: {e}"),
            BrowserError::Cdp(e) => write!(f, "browser protocol error: {e}"),
            BrowserError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BrowserError {}

/// A running browser and everything we need to stop it again.
pub struct Launched {
    /// The child process.
    child: Child,
    /// The port the browser chose. Kept for the log line and for tests.
    port: u16,
    /// The browser's WebSocket endpoint.
    pub ws_url: String,
    /// The browser's own product string, e.g. "Chrome/148.0.7778.96".
    pub product: String,
    /// Which browser this is.
    pub browser: Browser,
}

impl Launched {
    /// The port the browser is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Which browser this is.
    pub fn browser(&self) -> Browser {
        self.browser
    }

    /// The browser's own product string, e.g. "Chrome/148.0.7778.96".
    pub fn product(&self) -> &str {
        &self.product
    }
}

impl Drop for Launched {
    fn drop(&mut self) {
        // Ask nicely first; the caller may already have sent Browser.close.
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The profile is ours and it holds this browser's cookies and logins.
        // It stays between runs on purpose: a browser that forgets you every
        // launch is not a browser. It lives in our own cache directory.
    }
}

/// Start a browser and wait until it exposes its debugging port.
///
/// The profile lives under the user's cache directory, so it survives between
/// runs (a warm profile starts faster, and the browser keeps its logins) but
/// is still ours alone.
pub fn launch() -> Result<Launched, BrowserError> {
    launch_in(None)
}

/// Start a browser on a profile of its own.
///
/// `profile` is for a second browser that must not share ours; `None` uses
/// the suite's own profile, which keeps its cookies and logins.
pub fn launch_in(profile: Option<PathBuf>) -> Result<Launched, BrowserError> {
    let (browser, exe) = find_browser().ok_or(BrowserError::NotFound)?;

    let profile = match profile {
        Some(dir) => dir,
        None => profile_dir()?,
    };
    // A stale port file would send us to the previous run's port.
    let _ = std::fs::remove_file(profile.join("DevToolsActivePort"));
    // A lock left by a browser that is gone would make the new one exit
    // without starting.
    clear_stale_lock(&profile);

    let (mut child, port) = start(&exe, &profile, false)?;

    // Chromium refuses to start without a sandbox on hosts that block
    // unprivileged user namespaces (Ubuntu 23.10+ with AppArmor, most
    // containers). The sandbox is not optional on a real desktop, so it is
    // only ever dropped when the operator explicitly allows it.
    let port = match port {
        Ok(port) => port,
        Err(e) => {
            if !(sandbox_unavailable() && allow_no_sandbox()) {
                return Err(e);
            }
            log::warn!(
                "the browser could not use its sandbox; starting it without one \
                 because that was explicitly allowed"
            );
            // The first child is already dead; replace it, do not keep it.
            // The replacement reuses the profile, so the lock the dead
            // browser may have left behind goes too.
            clear_stale_lock(&profile);
            let (retry, port) = start(&exe, &profile, true)?;
            child = retry;
            port?
        }
    };

    // The WebSocket URL comes from the HTTP endpoint, not from the port file.
    //
    // `DevToolsActivePort` holds the port on line one and the browser guid on
    // line two, but that guid is only `/devtools/browser/<uuid>` when the
    // browser target is not discoverable. Chromium picks between the two based
    // on policy, so parsing it is a guess. `/json/version` always answers with
    // the full URL.
    let (ws_url, product) = ws_url_for(port)?;

    Ok(Launched {
        child,
        port,
        ws_url,
        product,
        browser,
    })
}

/// Start the browser and wait for its port.
///
/// `no_sandbox` drops the sandbox. It is only ever true when the caller has
/// established that the host cannot provide one and the operator allowed it.
fn start(
    exe: &Path,
    profile: &Path,
    no_sandbox: bool,
) -> Result<(Child, Result<u16, BrowserError>), BrowserError> {
    let mut command = Command::new(exe);
    command
        .arg("--headless=new")
        // Port 0: the browser picks a free port and writes it to the profile.
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        .arg("--disable-breakpad")
        .arg("--disable-component-update")
        .arg("--disable-sync")
        .arg("--metrics-recording-only")
        .arg("--password-store=basic")
        .arg("--use-mock-keychain")
        .arg("--window-size=1280,900")
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if no_sandbox {
        command.arg("--no-sandbox");
    }
    let mut child = command
        .spawn()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;
    let port = wait_for_port(profile, &mut child, Duration::from_secs(30));
    Ok((child, port))
}

/// Whether this host blocks the sandbox Chromium needs.
///
/// Unprivileged user namespaces are the modern sandbox mechanism; Ubuntu
/// 23.10 and later disable them through AppArmor unless a profile allows
/// them, and containers usually cannot create them either. Chromium then
/// aborts at startup with "No usable sandbox".
fn sandbox_unavailable() -> bool {
    #[cfg(target_os = "linux")]
    {
        // The kernel setting that AppArmor's restriction follows.
        if let Ok(value) = std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone")
            && value.trim() == "0"
        {
            return true;
        }
        // AppArmor's own switch, when the kernel exposes it.
        let path = "/proc/sys/kernel/apparmor_restrict_unprivileged_userns";
        if let Ok(value) = std::fs::read_to_string(path)
            && value.trim() == "1"
        {
            return true;
        }
        // Old kernels have neither file; the namespace is available there.
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Whether the operator allows running the browser without its sandbox.
///
/// Off unless set. A desktop user never needs this; a container or CI host
/// that cannot provide namespaces does.
fn allow_no_sandbox() -> bool {
    matches!(
        std::env::var("IRIDIUM_OFFICE_BROWSER_NO_SANDBOX").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// The browser's WebSocket URL and product string, read from `/json/version`.
fn ws_url_for(port: u16) -> Result<(String, String), BrowserError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(found) = read_version(port) {
            return Ok(found);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(BrowserError::BadEndpoint(format!(
        "the browser on port {port} did not answer /json/version"
    )))
}

/// One attempt at reading the browser's endpoint details.
fn read_version(port: u16) -> Option<(String, String)> {
    let mut body = ureq::get(&format!("http://127.0.0.1:{port}/json/version"))
        .call()
        .ok()?
        .into_body();
    // The version payload is a few hundred bytes; cap the read so a
    // misbehaving endpoint cannot make us allocate without bound.
    let response: String = body.with_config().limit(64 * 1024).read_to_string().ok()?;
    let value: serde_json::Value = serde_json::from_str(&response).ok()?;
    let url = value.get("webSocketDebuggerUrl")?.as_str()?;
    // Only ever talk to the browser we started, on the loopback interface.
    if !url.starts_with("ws://127.0.0.1:") && !url.starts_with("ws://localhost:") {
        return None;
    }
    let product = value
        .get("Browser")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Some((url.to_string(), product))
}

/// Wait for the browser to write its debugging port, and fail fast if it dies.
fn wait_for_port(
    profile: &Path,
    child: &mut Child,
    timeout: Duration,
) -> Result<u16, BrowserError> {
    let deadline = Instant::now() + timeout;
    let port_file = profile.join("DevToolsActivePort");
    while Instant::now() < deadline {
        // A dead browser must not look like a slow one.
        if let Ok(Some(status)) = child.try_wait() {
            return Err(BrowserError::LaunchFailed(if status.success() {
                // Chromium exits successfully when it hands the command line to
                // an instance that already holds the profile lock. That is the
                // "another browser is already running" case, not a crash.
                format!(
                    "another browser already holds the profile at {}; \
                     close it before starting the suite's browser",
                    profile.display()
                )
            } else {
                format!("the browser exited with {status} before it was ready")
            }));
        }
        if let Ok(text) = std::fs::read_to_string(&port_file)
            && let Some(port) = text
                .lines()
                .next()
                .and_then(|l| l.trim().parse::<u16>().ok())
            && port > 0
        {
            return Ok(port);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(BrowserError::LaunchFailed(
        "the browser did not report a debugging port in time".to_string(),
    ))
}

/// Clear a stale singleton lock from the profile directory.
///
/// Chromium locks the profile directory with a `SingletonLock` symlink whose
/// target names the process holding it. If that process is gone the lock is
/// stale, and the next browser to start would hand its command line to a
/// process that no longer exists and exit without opening a port.
///
/// The lock is only removed when the process it names is dead. A live lock
/// means a browser really is running on this profile, and that is the user's
/// to close.
fn clear_stale_lock(profile: &Path) {
    let lock = profile.join("SingletonLock");
    let Ok(target) = std::fs::read_link(&lock) else {
        return;
    };
    let Some(text) = target.to_str() else {
        return;
    };
    // The target is "<hostname>-<pid>".
    let Some(pid) = text.rsplit('-').next().and_then(|p| p.parse::<u32>().ok()) else {
        return;
    };
    if process_alive(pid) {
        return;
    }
    log::info!("clearing a stale browser profile lock from {pid}");
    let _ = std::fs::remove_file(&lock);
    let _ = std::fs::remove_file(profile.join("SingletonCookie"));
    let _ = std::fs::remove_file(profile.join("SingletonSocket"));
}

/// Whether a process is still running.
fn process_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        // `/proc/<pid>` exists exactly while the process does.
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // Signal 0 asks whether the process exists, without touching it.
        libc_kill(pid, 0) == 0
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// `kill(pid, 0)` — asks whether the process exists, without signalling it.
///
/// The block is `unsafe extern` because Rust 2024 requires it, and the call
/// inside it is the unsafe part. This code only builds on macOS and the BSDs,
/// so it is checked by a macOS cross-check rather than by the Linux gate.
#[cfg(all(unix, not(target_os = "linux")))]
fn libc_kill(pid: u32, signal: i32) -> i32 {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, signal) }
}

/// Our own profile directory, under the user's cache directory.
///
/// Chromium takes a lock on the profile directory and, if another instance
/// already holds it, hands the command line to that instance and exits
/// quietly. Two browsers sharing one profile would therefore break: the second
/// one exits without ever opening a port, and the suite is left with a
/// browser that never started.
///
/// So the profile is ours, in our own directory, and only one browser runs at
/// a time. The suite keeps a single browser engine for its lifetime; a second
/// one is not needed and would only fight the first over this directory.
fn profile_dir() -> Result<PathBuf, BrowserError> {
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Caches")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".cache")
            })
    };
    let dir = base.join("iridium-office").join("browser-profile");
    std::fs::create_dir_all(&dir)
        .map_err(|e| BrowserError::Other(format!("cannot create the browser profile: {e}")))?;
    Ok(dir)
}

/// A profile directory of its own, for a browser that must not share ours.
///
/// Chromium refuses to start a second instance on a profile another instance
/// holds. Two Workspace windows, or two tests, each need their own.
pub fn private_profile_dir(tag: &str) -> Result<PathBuf, BrowserError> {
    let dir = profile_dir()?.join(format!("instance-{tag}"));
    std::fs::create_dir_all(&dir)
        .map_err(|e| BrowserError::Other(format!("cannot create the browser profile: {e}")))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The browser list is sane: Brave first, and every candidate has a path.
    #[test]
    fn preference_order_and_candidates() {
        assert_eq!(PREFERENCE[0], Browser::Brave);
        for browser in PREFERENCE {
            assert!(
                !browser.candidates().is_empty(),
                "{browser:?} has no candidates"
            );
            assert!(!browser.label().is_empty());
        }
    }

    /// An override path is labelled correctly.
    #[test]
    fn override_is_labelled() {
        assert_eq!(
            browser_for(Path::new("/usr/bin/brave-browser")),
            Browser::Brave
        );
        assert_eq!(
            browser_for(Path::new("/usr/bin/microsoft-edge")),
            Browser::Edge
        );
        assert_eq!(
            browser_for(Path::new("/opt/google/chrome/chrome")),
            Browser::Chrome
        );
        assert_eq!(
            browser_for(Path::new("/usr/bin/chromium")),
            Browser::Chromium
        );
    }

    /// Launching works when a browser is installed, and the child is cleaned up.
    ///
    /// Skipped when no browser is installed: that is a supported state.
    #[test]
    fn launches_and_stops() {
        let Some((browser, _)) = find_browser() else {
            eprintln!("no Chromium-family browser installed; skipping");
            return;
        };
        // Its own profile: another test may be running a browser at the same
        // time, and Chromium will not start twice on one profile.
        let profile =
            private_profile_dir(&format!("test-{}", std::process::id())).expect("profile");
        let launched = launch_in(Some(profile)).expect("the browser starts");
        assert!(launched.port() > 0);
        assert!(launched.ws_url.contains("127.0.0.1"));
        assert!(!launched.ws_url.is_empty());
        assert_eq!(launched.browser(), browser);
        drop(launched);
    }

    /// A lock left by a browser that is gone is cleared; a lock held by a live
    /// process is left alone.
    #[cfg(unix)]
    #[test]
    fn a_stale_lock_is_cleared_and_a_live_one_is_not() {
        let dir = std::env::temp_dir().join(format!("workspace-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let lock = dir.join("SingletonLock");

        // A lock naming a process that cannot exist. `symlink_metadata` is
        // the right check here: `exists()` follows the link, and a link to a
        // dead process looks absent even while it is still there.
        std::os::unix::fs::symlink("host-4294967294", &lock).expect("symlink");
        clear_stale_lock(&dir);
        assert!(
            std::fs::symlink_metadata(&lock).is_err(),
            "a lock from a dead process is cleared"
        );

        // A lock naming this process, which is alive.
        std::os::unix::fs::symlink(format!("host-{}", std::process::id()), &lock).expect("symlink");
        clear_stale_lock(&dir);
        assert!(
            std::fs::symlink_metadata(&lock).is_ok(),
            "a lock from a live process is left alone"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
