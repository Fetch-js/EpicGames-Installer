use std::{ffi::CString, os::raw::c_char, path::PathBuf};

use tauri::Emitter;
use tokio::io::AsyncWriteExt;

// Replace this value with a direct HTTPS link to your ZIP archive.
const PAYLOAD_URL: &str = "PASTE_ZIP_URL_HERE";
const MINIMUM_REQUIRED_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    percent: u8,
    status: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiskSpace {
    available_bytes: u64,
    required_bytes: u64,
}

extern "C" {
    fn install_payload(
        source: *const c_char,
        destination: *const c_char,
        error_buffer: *mut c_char,
        error_buffer_size: usize,
    ) -> i32;
}

#[tauri::command]
async fn install_launcher(
    app: tauri::AppHandle,
    destination: String,
    create_shortcut: bool,
) -> Result<String, String> {
    let space = disk_space_for(&destination)?;
    if space.available_bytes < space.required_bytes {
        return Err("На выбранном диске недостаточно свободного места".to_string());
    }

    if !PAYLOAD_URL.starts_with("https://") {
        return Err("Укажите прямую HTTPS-ссылку на ZIP-архив в PAYLOAD_URL".to_string());
    }

    let temporary = tempfile::tempdir()
        .map_err(|error| format!("Не удалось создать временную папку: {error}"))?;
    let archive_path = temporary.path().join("payload.zip");
    let extracted_path = temporary.path().join("payload");

    download_payload(&app, &archive_path).await?;
    emit_progress(&app, 82, "Распаковка архива...");
    let archive_for_extract = archive_path.clone();
    let extracted_for_extract = extracted_path.clone();
    tokio::task::spawn_blocking(move || extract_zip(&archive_for_extract, &extracted_for_extract))
        .await
        .map_err(|error| format!("Ошибка задачи распаковки: {error}"))??;

    emit_progress(&app, 92, "Установка файлов...");
    let source = CString::new(extracted_path.to_string_lossy().as_bytes())
        .map_err(|_| "Некорректный путь к файлам установки".to_string())?;
    let destination_c = CString::new(destination.as_bytes())
        .map_err(|_| "Некорректный путь назначения".to_string())?;
    let mut error = vec![0_i8; 1024];

    let code = unsafe {
        install_payload(
            source.as_ptr(),
            destination_c.as_ptr(),
            error.as_mut_ptr(),
            error.len(),
        )
    };

    if code != 0 {
        let message = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        return Err(if message.is_empty() {
            format!("Не удалось установить приложение (код {code})")
        } else {
            message
        });
    }

    if create_shortcut {
        create_desktop_shortcut(&destination)?;
    }

    emit_progress(&app, 100, "Установка завершена");
    Ok(format!("Epic Games Launcher установлен в {destination}"))
}

#[tauri::command]
fn get_disk_space(app: tauri::AppHandle, destination: String) -> Result<DiskSpace, String> {
    let _ = app;
    disk_space_for(&destination)
}

fn disk_space_for(destination: &str) -> Result<DiskSpace, String> {
    let mut disk_path = PathBuf::from(destination);
    while !disk_path.exists() {
        if !disk_path.pop() {
            return Err("Не удалось определить диск для выбранного пути".to_string());
        }
    }

    let available_bytes = fs2::available_space(&disk_path)
        .map_err(|error| format!("Не удалось проверить свободное место: {error}"))?;
    Ok(DiskSpace {
        available_bytes,
        required_bytes: MINIMUM_REQUIRED_BYTES,
    })
}

async fn download_payload(app: &tauri::AppHandle, path: &std::path::Path) -> Result<(), String> {
    emit_progress(app, 3, "Подключение к серверу...");
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент: {error}"))?;
    let mut response = client
        .get(PAYLOAD_URL)
        .send()
        .await
        .map_err(|error| format!("Не удалось скачать архив: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Сервер вернул ошибку: {error}"))?;
    let total = response.content_length();
    let mut downloaded = 0_u64;
    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(|error| format!("Не удалось создать временный архив: {error}"))?;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Ошибка при загрузке архива: {error}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Не удалось записать архив: {error}"))?;
        downloaded += chunk.len() as u64;
        let percent = total
            .filter(|size| *size > 0)
            .map(|size| 5 + ((downloaded.saturating_mul(72) / size).min(72) as u8))
            .unwrap_or(40);
        emit_progress(app, percent, "Загрузка файлов...");
    }
    file.flush()
        .await
        .map_err(|error| format!("Не удалось сохранить архив: {error}"))?;

    Ok(())
}

fn extract_zip(
    archive_path: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let file = std::fs::File::open(archive_path)
        .map_err(|error| format!("Не удалось открыть скачанный архив: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("Файл не является корректным ZIP-архивом: {error}"))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Не удалось прочитать ZIP-архив: {error}"))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| "Архив содержит небезопасный путь".to_string())?;
        let output = destination.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&output)
                .map_err(|error| format!("Не удалось создать папку: {error}"))?;
            continue;
        }

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Не удалось создать папку: {error}"))?;
        }
        let mut output_file = std::fs::File::create(&output)
            .map_err(|error| format!("Не удалось распаковать файл: {error}"))?;
        std::io::copy(&mut entry, &mut output_file)
            .map_err(|error| format!("Не удалось распаковать файл: {error}"))?;
    }

    Ok(())
}

fn emit_progress(app: &tauri::AppHandle, percent: u8, status: &'static str) {
    let _ = app.emit("install-progress", InstallProgress { percent, status });
}

#[cfg(target_os = "windows")]
fn create_desktop_shortcut(destination: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let desktop = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .map_err(|_| "Не удалось определить рабочий стол".to_string())?
        .join("Desktop")
        .join("Epic Games Launcher.lnk");
    let target = PathBuf::from(destination).join("EpicGamesLauncher.exe");
    let shortcut_icon = PathBuf::from(destination).join("EpicGamesLauncher.ico");
    std::fs::write(&shortcut_icon, include_bytes!("../icons/icon.ico"))
        .map_err(|error| format!("Не удалось установить иконку ярлыка: {error}"))?;
    let escape = |value: &str| value.replace('`', "``").replace('\'', "''");
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.IconLocation='{},0';$s.Save()",
        escape(&desktop.to_string_lossy()),
        escape(&target.to_string_lossy()),
        escape(destination),
        escape(&shortcut_icon.to_string_lossy()),
    );
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000)
        .status()
        .map_err(|error| format!("Не удалось создать ярлык: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("Приложение установлено, но ярлык создать не удалось".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn create_desktop_shortcut(_destination: &str) -> Result<(), String> {
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![install_launcher, get_disk_space])
        .run(tauri::generate_context!())
        .expect("error while running installer");
}
