use anyhow::{Context, Result};
use download_cef::{CefFile, CefIndex, OsAndArch};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const CEF_VERSION: &str = "146.0.10";
const DEFAULT_DOWNLOAD_URL: &str = "https://cef-builds.spotifycdn.com";

fn looks_like_exported_cef(output: &Path) -> bool {
    output.join("archive.json").exists()
        && output.join("include").is_dir()
        && output.join("cmake").is_dir()
        && output.join(if cfg!(target_os = "windows") {
            "libcef.dll"
        } else {
            "libcef.so"
        })
        .is_file()
}

fn current_target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
}

fn export_cef(output: &Path, target: &str) -> Result<()> {
    if looks_like_exported_cef(output) {
        println!("Reusing existing CEF runtime at {}", output.display());
        return Ok(());
    }

    let parent = output
        .parent()
        .context("invalid CEF output directory")?
        .to_path_buf();

    if fs::exists(output)? {
        let old_output = parent.join(format!(
            "old_{}",
            output
                .file_name()
                .and_then(|part| part.to_str())
                .context("invalid CEF output directory")?
        ));
        fs::rename(output, &old_output)?;
        fs::remove_dir_all(old_output)?;
    }

    let os_arch = OsAndArch::try_from(target)?;
    let extracted_output = parent.join(os_arch.to_string());
    if fs::exists(&extracted_output)? {
        let old_output = parent.join(format!(
            "old_{}",
            extracted_output
                .file_name()
                .and_then(|part| part.to_str())
                .context("invalid extracted CEF directory")?
        ));
        fs::rename(&extracted_output, &old_output)?;
        fs::remove_dir_all(old_output)?;
    }

    let index = CefIndex::download_from(DEFAULT_DOWNLOAD_URL)?;
    let platform = index.platform(target)?;
    let version = platform.version(CEF_VERSION)?;

    let archive_path = version.download_archive_with_retry_from(
        DEFAULT_DOWNLOAD_URL,
        &parent,
        true,
        Duration::from_secs(15),
        3,
    )?;
    let archive = CefFile::try_from(archive_path.as_path())?;

    let extracted_dir = download_cef::extract_target_archive(target, &archive_path, &parent, true)?;
    if extracted_dir != extracted_output {
        anyhow::bail!(
            "extracted dir {:?} does not match expected dir {:?}",
            extracted_dir,
            extracted_output
        );
    }

    fs::remove_file(&archive_path)?;
    archive.write_archive_json(&extracted_output)?;

    if output != extracted_output {
        fs::rename(extracted_output, output)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: cargo run --package cef-exporter -- <output-dir>")?;

    export_cef(&output, current_target())
}
