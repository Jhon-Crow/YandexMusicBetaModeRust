//! Patcher module - handles downloading, extracting, and patching the Yandex Music app
//!
//! This is the main module that orchestrates the entire patching process:
//! 1. Download the installer from Yandex servers
//! 2. Extract the installer using 7z
//! 3. Extract the app.asar archive
//! 4. Apply all patches to the JavaScript/JSON files
//! 5. Rebuild the application

use crate::api::{download_build, AppBuild};
use crate::patches;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// Process a build: download, extract, patch, and rebuild
pub async fn process_build(
    build: &AppBuild,
    output_dir: &str,
    auto_devtools: bool,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    let build_dir = PathBuf::from(output_dir).join(&build.version);
    let temp_dir = build_dir.join("temp");
    let build_binary_path = temp_dir.join("build.exe");
    let extract_dir = temp_dir.join("extracted");
    let build_source_dir = build_dir.join("src");
    let build_modded_dir = build_dir.join("mod");

    // Clean up any existing build directory
    if build_dir.exists() {
        info!("Removing existing build directory: {:?}", build_dir);
        fs::remove_dir_all(&build_dir)?;
    }

    // Create directories
    fs::create_dir_all(&build_dir)?;
    fs::create_dir_all(&extract_dir)?;
    fs::create_dir_all(&build_source_dir)?;
    fs::create_dir_all(&build_modded_dir)?;

    update_progress(progress, 5, "Downloading build...");
    info!("[1] Downloading build {}", build.version);

    download_build(build, build_binary_path.to_str().unwrap()).await?;
    info!("Download complete");

    update_progress(progress, 20, "Extracting installer...");
    info!(
        "[2] Extracting build {} to {:?}",
        build.version, extract_dir
    );

    extract_installer(&build_binary_path, &extract_dir)?;
    info!("Extraction complete");

    update_progress(progress, 35, "Locating and extracting app.asar...");
    info!("[3] Finding and extracting app.asar");

    let app_asar_path = extract_dir.join("resources").join("app.asar");
    let app_icon_path = extract_dir
        .join("resources")
        .join("assets")
        .join("icon.ico");

    if !app_asar_path.exists() {
        anyhow::bail!("app.asar not found at {:?}", app_asar_path);
    }
    info!("Found app.asar at {:?}", app_asar_path);

    // Copy icon if it exists
    if app_icon_path.exists() {
        fs::copy(&app_icon_path, build_dir.join("icon.ico"))?;
        info!("Copied app icon");
    }

    // Extract app.asar
    extract_asar(&app_asar_path, &build_source_dir)?;
    info!("Extracted app.asar");

    update_progress(progress, 45, "Cleaning up temp files...");
    info!("[4] Cleaning up temporary files");

    fs::remove_dir_all(&temp_dir)?;
    info!("Cleanup complete");

    update_progress(progress, 50, "Copying sources...");
    info!("[5] Copying sources before modding");

    copy_dir_all(&build_source_dir, &build_modded_dir)?;
    info!("Copy complete");

    update_progress(progress, 55, "Applying patches...");
    info!("[6] Patching application");

    apply_patches(&build_modded_dir, auto_devtools)?;
    info!("Patching complete");

    update_progress(progress, 80, "Creating mod files...");
    info!("[7] Creating mod files");

    create_mod_files(&build_modded_dir)?;
    info!("Mod files created");

    update_progress(progress, 90, "Injecting mod into HTML...");
    info!("[8] Injecting mod into HTML files");

    inject_mod_into_html(&build_modded_dir)?;
    info!("HTML injection complete");

    update_progress(progress, 100, "Done!");
    info!("Build {} patched successfully!", build.version);
    info!("Output directory: {:?}", build_modded_dir);

    Ok(())
}

fn update_progress(progress: Option<&ProgressBar>, pos: u64, msg: &str) {
    if let Some(pb) = progress {
        pb.set_position(pos);
        pb.set_message(msg.to_string());
    }
}

/// Extract the installer using 7z or a built-in extractor
fn extract_installer(installer_path: &Path, output_dir: &Path) -> Result<()> {
    // Try using 7z command if available
    let result = Command::new("7z")
        .args(["x", "-y", "-o"])
        .arg(output_dir)
        .arg(installer_path)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("7z extraction successful");
                return Ok(());
            }
            warn!("7z failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            warn!("7z not found, trying alternative methods: {}", e);
        }
    }

    // Try using 7zz (7-Zip on some systems)
    let result = Command::new("7zz")
        .args(["x", "-y", &format!("-o{}", output_dir.display())])
        .arg(installer_path)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("7zz extraction successful");
                return Ok(());
            }
            warn!("7zz failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            warn!("7zz not found: {}", e);
        }
    }

    // Try using p7zip
    let result = Command::new("p7zip")
        .args(["-d", "-k"])
        .arg(installer_path)
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            debug!("p7zip extraction successful");
            return Ok(());
        }
    }

    // If all else fails, try using the zip crate (may work for some installers)
    match extract_with_zip(installer_path, output_dir) {
        Ok(_) => return Ok(()),
        Err(e) => {
            warn!("Zip extraction failed: {}", e);
        }
    }

    anyhow::bail!(
        "Failed to extract installer. Please install 7z/7zip and ensure it's in PATH.\n\
         On Windows: Download from https://www.7-zip.org/\n\
         On Linux: apt install p7zip-full\n\
         On macOS: brew install p7zip"
    )
}

/// Try to extract using the zip crate
fn extract_with_zip(archive_path: &Path, output_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => output_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

/// Extract an ASAR archive
fn extract_asar(asar_path: &Path, output_dir: &Path) -> Result<()> {
    // Try using the asar command-line tool
    let result = Command::new("asar")
        .args(["extract"])
        .arg(asar_path)
        .arg(output_dir)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("asar extraction successful");
                return Ok(());
            }
            warn!(
                "asar command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            warn!("asar command not found: {}", e);
        }
    }

    // Try using npx asar
    let result = Command::new("npx")
        .args(["asar", "extract"])
        .arg(asar_path)
        .arg(output_dir)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("npx asar extraction successful");
                return Ok(());
            }
            warn!(
                "npx asar failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            warn!("npx not found: {}", e);
        }
    }

    // Try using the asar crate
    match extract_asar_native(asar_path, output_dir) {
        Ok(_) => return Ok(()),
        Err(e) => {
            warn!("Native asar extraction failed: {}", e);
        }
    }

    anyhow::bail!(
        "Failed to extract app.asar. Please install asar:\n\
         npm install -g asar\n\
         Or ensure Node.js/npx is in PATH."
    )
}

/// Native ASAR extraction using the asar crate
fn extract_asar_native(asar_path: &Path, output_dir: &Path) -> Result<()> {
    use asar::AsarReader;

    let asar_data = fs::read(asar_path)?;
    let reader = AsarReader::new(&asar_data, Some(asar_path.to_path_buf()))
        .context("Failed to read ASAR archive")?;

    for (path, file) in reader.files() {
        let output_path = output_dir.join(path);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = file.data();
        if !data.is_empty() {
            fs::write(&output_path, data)?;
        }
    }

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Apply all patches to the modded directory
fn apply_patches(modded_dir: &Path, auto_devtools: bool) -> Result<()> {
    let package_json_path = modded_dir.join("package.json");
    let config_js_path = modded_dir.join("main").join("config.js");
    let main_js_path = modded_dir.join("main").join("index.js");
    let preload_js_path = modded_dir.join("main").join("lib").join("preload.js");
    let create_window_js_path = modded_dir.join("main").join("lib").join("createWindow.js");
    let system_menu_js_path = modded_dir.join("main").join("lib").join("systemMenu.js");

    // Patch package.json
    if package_json_path.exists() {
        info!("Patching package.json");
        let content = fs::read_to_string(&package_json_path)?;
        let patched = patches::patch_package_json(&content)?;
        fs::write(&package_json_path, patched)?;
    }

    // Patch config.js
    if config_js_path.exists() {
        info!("Patching config.js");
        let content = fs::read_to_string(&config_js_path)?;
        let patched = patches::patch_config_js(&content);
        fs::write(&config_js_path, patched)?;
    }

    // Patch systemMenu.js
    if system_menu_js_path.exists() {
        info!("Patching systemMenu.js");
        let content = fs::read_to_string(&system_menu_js_path)?;
        let patched = patches::patch_system_menu_js(&content);
        fs::write(&system_menu_js_path, patched)?;
    }

    // Patch createWindow.js
    if create_window_js_path.exists() {
        info!("Patching createWindow.js");
        let content = fs::read_to_string(&create_window_js_path)?;
        let patched = patches::patch_create_window_js(&content, auto_devtools);
        fs::write(&create_window_js_path, patched)?;
    }

    // Patch main.js (index.js)
    if main_js_path.exists() {
        info!("Patching index.js");
        let content = fs::read_to_string(&main_js_path)?;
        let mut patched = patches::patch_main_js(&content);

        // Append mod main.js
        patched.push_str("\n\n// YandexMusicMod main.js\n");
        patched.push_str(patches::MOD_MAIN_JS);

        fs::write(&main_js_path, patched)?;
    }

    // Patch preload.js
    if preload_js_path.exists() {
        info!("Patching preload.js");
        let content = fs::read_to_string(&preload_js_path)?;
        let mut patched = content;

        // Append mod preload.js
        patched.push_str("\n\n// YandexMusicMod preload.js\n");
        patched.push_str(patches::MOD_PRELOAD_JS);

        fs::write(&preload_js_path, patched)?;
    }

    // Remove splash screen if it exists
    let splash_screen_path = modded_dir.join("app").join("media").join("splash_screen");
    if splash_screen_path.exists() {
        info!("Removing splash screen");
        fs::remove_dir_all(&splash_screen_path)?;
    }

    Ok(())
}

/// Create mod files in the app directory
fn create_mod_files(modded_dir: &Path) -> Result<()> {
    let mod_dir = modded_dir.join("app").join("yandexMusicMod");
    fs::create_dir_all(&mod_dir)?;

    // Create renderer.js
    fs::write(mod_dir.join("renderer.js"), patches::MOD_RENDERER_JS)?;

    // Create renderer.css
    fs::write(mod_dir.join("renderer.css"), patches::MOD_RENDERER_CSS)?;

    info!("Created mod files in {:?}", mod_dir);
    Ok(())
}

/// Inject mod scripts into all HTML files
fn inject_mod_into_html(modded_dir: &Path) -> Result<()> {
    let app_dir = modded_dir.join("app");

    for entry in WalkDir::new(&app_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
    {
        let path = entry.path();
        info!("Patching HTML: {:?}", path);

        let content = fs::read_to_string(path)?;
        let patched = patches::patch_html(&content);
        fs::write(path, patched)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_dir_all() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("test.txt"), "hello").unwrap();

        copy_dir_all(&src, &dst).unwrap();

        assert!(dst.join("test.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("test.txt")).unwrap(), "hello");
    }
}
