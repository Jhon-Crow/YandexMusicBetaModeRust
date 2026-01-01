//! Patches module - contains all JavaScript/JSON patches for the Yandex Music app
//!
//! This module contains the actual code modifications that will be applied
//! to the extracted Yandex Music application files.

use serde_json::{json, Value};

/// Blocked analytics and telemetry URLs
pub const BLOCKED_ANALYTICS_URLS: &[&str] = &[
    "https://yandex.ru/clck/*",
    "https://mc.yandex.ru/*",
    "https://api.music.yandex.net/dynamic-pages/trigger/*",
    "https://log.strm.yandex.ru/*",
    "https://api.acquisition-gwe.plus.yandex.net/*",
    "https://api.events.plus.yandex.net/*",
    "https://events.plus.yandex.net/*",
    "https://plus.yandex.net/*",
    "https://yandex.ru/ads/*",
    "https://strm.yandex.ru/ping",
];

/// Banned headers to remove from API requests
pub const BANNED_HEADERS: &[&str] = &["x-yandex-music-device", "x-request-id"];

/// Banned dependencies to remove from package.json
pub const BANNED_DEPENDENCIES: &[&str] = &["@yandex-chats/signer"];

/// Patch the package.json file with mod settings
pub fn patch_package_json(content: &str) -> anyhow::Result<String> {
    let mut json: Value = serde_json::from_str(content)?;

    // Remove banned dependencies
    if let Some(deps) = json.get_mut("dependencies") {
        if let Some(obj) = deps.as_object_mut() {
            for banned in BANNED_DEPENDENCIES {
                obj.remove(*banned);
            }
        }
    }
    if let Some(dev_deps) = json.get_mut("devDependencies") {
        if let Some(obj) = dev_deps.as_object_mut() {
            for banned in BANNED_DEPENDENCIES {
                obj.remove(*banned);
            }
        }
    }

    // Update common config
    if let Some(common) = json.get_mut("common") {
        if let Some(obj) = common.as_object_mut() {
            obj.insert(
                "REFRESH_EVENT_TRIGGER_TIME_MS".to_string(),
                json!(999_999_999),
            );
            obj.insert("UPDATE_POLL_INTERVAL_MS".to_string(), json!(999_999_999));
            obj.insert("SUPPORT_URL".to_string(), json!("<empty>"));
        }
    }

    // Update package metadata
    json["name"] = json!("YandexMusicMod");
    json["author"] =
        json!("YandexMusicBetaModeFastLP [github.com/Jhon-Crow/YandexMusicBetaModeFastLP]");

    // Update meta information
    if let Some(meta) = json.get_mut("meta") {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("PRODUCT_NAME".to_string(), json!("Yandex Music Mod"));
            obj.insert(
                "PRODUCT_NAME_LOCALIZED".to_string(),
                json!("Yandex Music Mod"),
            );
            obj.insert("APP_ID".to_string(), json!("ru.yandex.desktop.music.mod"));
            obj.insert(
                "COPYRIGHT".to_string(),
                json!("YandexMusicBetaModeFastLP [github.com/Jhon-Crow/YandexMusicBetaModeFastLP]"),
            );
            obj.insert(
                "TRADEMARK".to_string(),
                json!("YandexMusicBetaModeFastLP [github.com/Jhon-Crow/YandexMusicBetaModeFastLP]"),
            );
        }
    }

    // Update app config
    if let Some(app_config) = json.get_mut("appConfig") {
        if let Some(obj) = app_config.as_object_mut() {
            obj.insert("enableDevTools".to_string(), json!(true));
            obj.insert("enableAutoUpdate".to_string(), json!(false));
            obj.insert("enableUpdateByProbability".to_string(), json!(false));
            obj.insert("systemDefaultLanguage".to_string(), json!("ru"));
        }
    }

    // Add build configuration
    json["build"] = json!({
        "appId": "ru.yandex.desktop.music.mod",
        "productName": "Яндекс Музыка",
        "win": {
            "icon": "assets/icon.ico"
        },
        "mac": {
            "icon": "assets/icon.ico"
        },
        "linux": {
            "icon": "assets/icon.png"
        },
        "extraResources": [{
            "from": "assets/",
            "to": "assets/",
            "filter": ["**/*"]
        }]
    });

    Ok(serde_json::to_string_pretty(&json)?)
}

/// Patch config.js to enable devtools and disable auto-update
pub fn patch_config_js(content: &str) -> String {
    content
        .replace("enableDevTools: false", "enableDevTools: true")
        .replace("enableDevTools:false", "enableDevTools: true")
        .replace("enableAutoUpdate: true", "enableAutoUpdate: false")
        .replace("enableAutoUpdate:true", "enableAutoUpdate: false")
}

/// Generate the settings reader code for system menu
pub fn generate_settings_reader_js() -> String {
    r#"
const fs = require("fs");
const path = require("path");
const electron = require("electron");
const appFolder = electron.app.getPath("userData");
const settingsFilePath = path.join(appFolder, "mod_settings.json");
let enableSystemToolbar = false;
try {
  enableSystemToolbar = JSON.parse(fs.readFileSync(settingsFilePath, "utf8"))["devtools/systemToolbar"];
} catch (e) {}
"#.to_string()
}

/// Patch systemMenu.js to read settings
pub fn patch_system_menu_js(content: &str) -> String {
    let settings_reader = generate_settings_reader_js();
    format!(
        "{}\n{}",
        settings_reader,
        content.replace(
            "deviceInfo_js_1.devicePlatform === platform_js_1.Platform.MACOS",
            "enableSystemToolbar"
        )
    )
}

/// Patch createWindow.js for devtools and window settings
pub fn patch_create_window_js(content: &str, auto_devtools: bool) -> String {
    let settings_reader = generate_settings_reader_js();

    let mut result = format!(
        "{}\n{}",
        settings_reader,
        content
            .replace("config_js_1.config.app.enableDevTools", "true")
            .replace(
                "titleBarStyle: 'hidden'",
                "titleBarStyle: !enableSystemToolbar && 'hidden'"
            )
            .replace(
                "titleBarStyle:'hidden'",
                "titleBarStyle: !enableSystemToolbar && 'hidden'"
            )
            .replace("minWidth: 768", "minWidth: 360")
            .replace("minHeight: 650", "minHeight: 550")
            .replace("show: false", "show: true")
    );

    if auto_devtools {
        result = result.replace(
            "return window",
            "window.webContents.openDevTools();\nreturn window",
        );
    }

    result
}

/// Generate the analytics blocking code for main.js
pub fn generate_analytics_blocker_js() -> String {
    let urls_json = serde_json::to_string(BLOCKED_ANALYTICS_URLS).unwrap();
    let banned_headers_json = serde_json::to_string(BANNED_HEADERS).unwrap();

    format!(
        r#"
const {{ session }} = require("electron");
session.defaultSession.webRequest.onBeforeRequest(
  {{
    urls: {urls},
  }},
  (details, callback) => {{
    callback({{ cancel: true }});
  }},
);

session.defaultSession.webRequest.onBeforeSendHeaders(
  {{
    urls: ["https://api.music.yandex.net/*"],
  }},
  (details, callback) => {{
    const bannedHeaders = {headers};
    bannedHeaders.forEach((header) => {{
      details.requestHeaders[header] = undefined;
    }});
    callback({{ requestHeaders: details.requestHeaders }});
  }},
);
"#,
        urls = urls_json,
        headers = banned_headers_json
    )
}

/// Patch main.js (index.js) with analytics blocker and mod code
pub fn patch_main_js(content: &str) -> String {
    let analytics_blocker = generate_analytics_blocker_js();

    content.replace(
        "createWindow)();",
        &format!("createWindow)();{}", analytics_blocker),
    )
}

/// Patch HTML files to inject the mod renderer script
pub fn patch_html(content: &str) -> String {
    content.replace(
        "<head>",
        r#"<head><script src="/yandexMusicMod/renderer.js"></script>
        <link rel="stylesheet" href="/yandexMusicMod/renderer.css">"#,
    )
}

/// The main.js mod code that handles IPC, settings, and downloads
pub const MOD_MAIN_JS: &str = r#"
const electron = require("electron");
const fs = require("fs");
const path = require("path");
const process = require("process");

const appFolder = electron.app.getPath("userData");
const settingsFilePath = path.join(appFolder, "mod_settings.json");
const defaultDownloadPath = path.join(appFolder, "Downloads");

// Create settings directory
fs.mkdir(appFolder, { recursive: true }, (err) => {
  if (err) return console.error(err);
  console.log("mod_settings directory created successfully!");
});

// Create default download directory
fs.mkdir(defaultDownloadPath, { recursive: true }, (err) => {
  if (err) return console.error(err);
  console.log("Default download directory created successfully!");
});

// Initialize settings file
if (!fs.existsSync(settingsFilePath)) {
  const initialSettings = {
    downloadFolderPath: defaultDownloadPath,
  };
  fs.writeFileSync(settingsFilePath, JSON.stringify(initialSettings, null, 2));
} else {
  try {
    const settings = JSON.parse(fs.readFileSync(settingsFilePath, "utf8"));
    if (!settings.downloadFolderPath) {
      settings.downloadFolderPath = defaultDownloadPath;
      fs.writeFileSync(settingsFilePath, JSON.stringify(settings, null, 2));
    }
  } catch (e) {
    const initialSettings = {
      downloadFolderPath: defaultDownloadPath,
    };
    fs.writeFileSync(settingsFilePath, JSON.stringify(initialSettings, null, 2));
  }
}

// IPC handlers for settings
electron.ipcMain.handle("yandexMusicMod.getStorageValue", (_ev, key) => {
  const settings = fs.readFileSync(settingsFilePath, "utf8") || "{}";
  const parsed = JSON.parse(settings);
  return parsed[key] !== undefined ? parsed[key] : null;
});

electron.ipcMain.on("yandexMusicMod.setStorageValue", (_ev, key, value) => {
  const settings = JSON.parse(fs.readFileSync(settingsFilePath, "utf8"));
  settings[key] = value;
  fs.writeFileSync(settingsFilePath, JSON.stringify(settings, null, 2));

  electron.BrowserWindow.getAllWindows().forEach((window) =>
    window.webContents.send("yandexMusicMod.storageValueUpdated", key, value),
  );
});

// Folder selection dialog
electron.ipcMain.handle("yandexMusicMod.selectDownloadFolder", async (_ev) => {
  const result = await electron.dialog.showOpenDialog({
    properties: ["openDirectory"],
    title: "Select download folder",
  });

  if (result.canceled || !result.filePaths.length) {
    return { success: false, path: null };
  }

  return { success: true, path: result.filePaths[0] };
});

// Open folder
electron.ipcMain.handle("yandexMusicMod.openFolder", async (_ev, folderPath) => {
  try {
    require("child_process").exec(`start "" "${folderPath}"`);
    return { success: true };
  } catch (error) {
    console.error("Failed to open folder:", error);
    return { success: false, error: error.message };
  }
});

// Open download directory
electron.ipcMain.on("yandexMusicMod.openDownloadDirectory", (_ev) => {
  let saveFolder;
  try {
    const settings = JSON.parse(fs.readFileSync(settingsFilePath, "utf8"));
    saveFolder = settings.downloadFolderPath || process.env.USERPROFILE + "\\YandexMod Download";
  } catch (e) {
    saveFolder = process.env.USERPROFILE + "\\YandexMod Download";
  }
  require("child_process").exec('start "" "' + saveFolder + '"');
});

console.log("YandexMusicMod main.js loaded successfully!");
"#;

/// The preload.js mod code
pub const MOD_PRELOAD_JS: &str = r#"
const { contextBridge, ipcRenderer } = require("electron");

// Expose mod API to the renderer process
contextBridge.exposeInMainWorld("yandexMusicMod", {
  getStorageValue: (key) => ipcRenderer.invoke("yandexMusicMod.getStorageValue", key),
  setStorageValue: (key, value) => ipcRenderer.send("yandexMusicMod.setStorageValue", key, value),
  onStorageValueUpdated: (callback) => {
    ipcRenderer.on("yandexMusicMod.storageValueUpdated", (event, key, value) => {
      callback(key, value);
    });
  },
  selectDownloadFolder: () => ipcRenderer.invoke("yandexMusicMod.selectDownloadFolder"),
  openFolder: (folderPath) => ipcRenderer.invoke("yandexMusicMod.openFolder", folderPath),
  openDownloadDirectory: () => ipcRenderer.send("yandexMusicMod.openDownloadDirectory"),
});

console.log("YandexMusicMod preload.js loaded successfully!");
"#;

/// The renderer.js mod code (minimal placeholder)
pub const MOD_RENDERER_JS: &str = r#"
(function() {
  console.log("YandexMusicMod renderer.js loaded!");

  // Wait for the page to load
  window.addEventListener("load", function() {
    console.log("YandexMusicMod: Page loaded");

    // Add mod indicator
    const modIndicator = document.createElement("div");
    modIndicator.style.cssText = "position:fixed;bottom:10px;right:10px;padding:5px 10px;background:rgba(0,0,0,0.7);color:#fff;border-radius:5px;font-size:12px;z-index:9999;";
    modIndicator.textContent = "YandexMusicMod";
    document.body.appendChild(modIndicator);

    // Hide indicator after 5 seconds
    setTimeout(() => {
      modIndicator.style.opacity = "0";
      modIndicator.style.transition = "opacity 0.5s";
      setTimeout(() => modIndicator.remove(), 500);
    }, 5000);
  });
})();
"#;

/// The renderer.css mod styles
pub const MOD_RENDERER_CSS: &str = r#"
/* YandexMusicMod custom styles */

/* Hide upgrade banners */
.upgrade-banner,
.plus-promo,
.subscription-promo {
  display: none !important;
}

/* Custom scrollbar */
::-webkit-scrollbar {
  width: 8px;
}

::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.1);
}

::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.3);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.5);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_config_js() {
        let input = "enableDevTools: false, enableAutoUpdate: true";
        let output = patch_config_js(input);
        assert!(output.contains("enableDevTools: true"));
        assert!(output.contains("enableAutoUpdate: false"));
    }

    #[test]
    fn test_patch_html() {
        let input = "<html><head><title>Test</title></head></html>";
        let output = patch_html(input);
        assert!(output.contains("yandexMusicMod/renderer.js"));
        assert!(output.contains("yandexMusicMod/renderer.css"));
    }

    #[test]
    fn test_patch_package_json() {
        let input = r#"{
            "name": "yandex-music",
            "dependencies": {"@yandex-chats/signer": "1.0.0", "other": "2.0.0"},
            "common": {"OLD": 100},
            "meta": {"PRODUCT_NAME": "Yandex Music"},
            "appConfig": {"enableDevTools": false}
        }"#;

        let output = patch_package_json(input).unwrap();
        let json: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["name"], "YandexMusicMod");
        assert!(json["dependencies"]["@yandex-chats/signer"].is_null());
        assert!(!json["dependencies"]["other"].is_null());
    }
}
