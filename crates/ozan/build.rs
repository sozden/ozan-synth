//! On Windows (MSVC), puts the icon and the version details into ozan.exe, so
//! Explorer, the taskbar and the installer show them. Needs `rc.exe` from the
//! Windows SDK (installed with the Visual Studio build tools); without it the
//! program still builds, only without an icon.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=RC");
    let windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !windows || !msvc {
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found (Windows SDK): ozan.exe gets no icon");
        return;
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("assets").join("icon.ico");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = version.split(['.', '-']).map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));

    // Numbers instead of the winver.h names, so rc needs no include path.
    let script = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3f
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "OZAN Synth"
      VALUE "FileDescription", "OZAN Synth"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ozan"
      VALUE "LegalCopyright", "GPL-3.0-or-later"
      VALUE "OriginalFilename", "ozan.exe"
      VALUE "ProductName", "OZAN Synth"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = icon.display().to_string().replace('\\', "\\\\"),
    );
    let rc_file = out.join("ozan.rc");
    let res_file = out.join("ozan.res");
    std::fs::write(&rc_file, script).unwrap();
    let status = Command::new(&rc).arg("/nologo").arg("/fo").arg(&res_file).arg(&rc_file).status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", res_file.display()),
        Ok(s) => println!("cargo:warning=rc.exe failed ({s}): ozan.exe gets no icon"),
        Err(e) => println!("cargo:warning=rc.exe could not run ({e}): ozan.exe gets no icon"),
    }
}

/// `RC` if set, otherwise the newest x64 rc.exe of the Windows 10/11 SDK.
fn find_rc() -> Option<PathBuf> {
    if let Some(rc) = env::var_os("RC") {
        return Some(PathBuf::from(rc));
    }
    let kits = env::var_os("ProgramFiles(x86)").map(PathBuf::from)?.join("Windows Kits").join("10").join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&kits).ok()?.flatten().map(|e| e.path()).filter(|p| p.join("x64").join("rc.exe").is_file()).collect();
    versions.sort_by_key(|p| version_key(p));
    versions.pop().map(|p| p.join("x64").join("rc.exe"))
}

/// "10.0.26100.0" as numbers, so 10.0.26100 sorts after 10.0.9600.
fn version_key(p: &Path) -> Vec<u32> {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("").split('.').map(|n| n.parse().unwrap_or(0)).collect()
}
