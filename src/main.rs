use runner2::{
    config::{self, LauncherData},
    file::FileManager,
    launcher::Launcher,
    manifest::ManifestManager,
    network::NetworkManager,
    ui::{RunnerApp, UiMessage},
    Result,
};

use eframe::egui::ViewportBuilder;
use log::{info, warn, error};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use tokio::runtime::Runtime;
use tempfile;
use std::fs::OpenOptions;
use std::env;
use directories::BaseDirs;

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 100.0;

fn get_log_file_path() -> Result<PathBuf> {
    if cfg!(target_os = "macos") {
        let base_dirs = BaseDirs::new()
            .ok_or_else(|| runner2::Error::FileSystem("Could not determine base directories".into()))?;
        
        let log_dir = base_dirs
            .data_dir()
            .join("PatchKit")
            .join("Apps");
            
        // Create the directory if it doesn't exist
        std::fs::create_dir_all(&log_dir)?;
        
        Ok(log_dir.join("launcher-log.txt"))
    } else {
        // For Windows and Linux, use the directory where the executable is located
        let exe_dir = env::current_exe()?
            .parent()
            .ok_or_else(|| runner2::Error::Other("Failed to get executable directory".into()))?
            .to_path_buf();
            
        Ok(exe_dir.join("launcher-log.txt"))
    }
}

#[cfg(windows)]
fn is_elevated() -> bool {
    use winapi::um::winnt::TOKEN_ELEVATION;
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::winnt::TOKEN_QUERY;
    use std::ptr::null_mut;

    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        
        GetTokenInformation(
            token,
            winapi::um::winnt::TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut size,
        ) != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn is_elevated() -> bool {
    // On Unix systems, we don't need elevation for writing next to the executable
    true
}

#[cfg(windows)]
fn restart_as_admin() -> Result<()> {
    use std::process::Command;
    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_NORMAL;
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    let exe_path = env::current_exe()
        .map_err(|e| runner2::Error::Other(format!("Failed to get executable path: {}", e)))?;

    let operation: Vec<u16> = OsStr::new("runas\0").encode_wide().collect();
    let file: Vec<u16> = exe_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let parameters: Vec<u16> = OsStr::new("\0").encode_wide().collect();
    let directory: Vec<u16> = exe_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            directory.as_ptr(),
            SW_NORMAL,
        );
    }

    std::process::exit(0);
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get the log file path
    let log_path = get_log_file_path()?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    // If we failed to create/open the log file and we're on Windows and not elevated
    #[cfg(windows)]
    if log_file.is_err() && !is_elevated() {
        // Can't use info! here as logger isn't initialized yet
        eprintln!("Failed to create log file, attempting to restart with admin privileges");
        restart_as_admin()?;
        return Ok(());
    }

    // Set up logging to both stderr and file if available
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    );
    builder.format_timestamp_millis();

    // If we successfully opened the log file, add it as a target
    if let Ok(log_file) = log_file {
        builder.target(env_logger::Target::Pipe(Box::new(log_file)));
    }

    builder.init();

    info!("Starting PatchKit Runner");

    let options = eframe::NativeOptions {
        default_theme: eframe::Theme::Dark,
        viewport: ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    info!("Initializing UI");
    eframe::run_native(
        "PatchKit Runner",
        options,
        Box::new(|cc| {
            let app = RunnerApp::new(cc);
            let sender = app.sender();
            
            info!("Spawning runner logic thread");
            std::thread::spawn(move || {
                if let Err(e) = Runtime::new()
                    .unwrap()
                    .block_on(run_launcher(sender.clone()))
                {
                    error!("Runner error: {}", e);
                    let _ = sender.send(UiMessage::ShowError(e.to_string()));
                }
            });
            
            Box::new(app)
        }),
    )
    .map_err(|e| runner2::Error::Other(e.to_string()))?;

    Ok(())
}

async fn run_launcher(sender: Sender<UiMessage>) -> Result<()> {
    // Initialize components
    info!("Initializing components");
    let network = NetworkManager::new();
    
    // Read the .dat file first to get the app secret
    info!("Reading launcher.dat file");
    let dat_file = std::fs::File::open("launcher.dat")
        .map_err(|e| {
            error!("Failed to open launcher.dat: {}", e);
            runner2::Error::DatFile(format!("Failed to open launcher.dat: {}", e))
        })?;
    let launcher_data = LauncherData::from_binary(dat_file)?;
    info!("Successfully read launcher.dat");
    
    // Initialize file manager with the first 8 chars of app secret
    let app_slug = &launcher_data.app_secret[..8];
    let mut file_manager = FileManager::new(app_slug)?;
    let launcher = Launcher::new();
    let extract_path = FileManager::get_patcher_dir(app_slug)?;

    // Check network connection
    info!("Checking network connection");
    sender.send(UiMessage::SetStatus("Checking network connection...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;

    if !network.check_connection().await? {
        return Err(runner2::Error::Other("No internet connection".into()));
    }
    info!("Network connection established");

    // Get app info to determine the correct patcher secret
    info!("Fetching app info");
    sender.send(UiMessage::SetStatus("Fetching app info...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let app_info = network.get_app_info(&launcher_data.app_secret).await?;
    info!("Got app info: {:?}", app_info);

    // Determine which patcher secret to use
    let patcher_secret = app_info.patcher_secret
        .unwrap_or_else(|| launcher_data.patcher_secret.clone());
    info!("Using patcher secret: {}", patcher_secret);

    // Get latest version
    info!("Fetching latest version");
    sender.send(UiMessage::SetStatus("Fetching latest version...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let version = network.get_latest_version(&patcher_secret).await?;
    info!("Latest version: {}", version);

    // Check if we need to update
    info!("Checking if update is needed");
    if !file_manager.needs_update(&version, &patcher_secret)? {
        info!("Already have the latest version {}, skipping update", version);
        
        // Launch the existing version
        launch_from_manifest(&extract_path, &file_manager, &launcher_data, &launcher, &sender)?;
        return Ok(());
    }
    info!("Update needed to version {}", version);

    // Get download URLs
    info!("Getting download URLs");
    sender.send(UiMessage::SetStatus("Getting download URLs...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let content_urls = network
        .get_content_urls(&patcher_secret, &version)
        .await?;

    if let Some(content) = content_urls.first() {
        info!("Found content URL: {}", content.url);
        
        // Download launcher package
        info!("Downloading launcher package");
        sender.send(UiMessage::SetStatus("Downloading launcher...".into()))
            .map_err(|e| runner2::Error::Other(e.to_string()))?;
        
        // Create a temporary file for download
        let temp_file = tempfile::Builder::new()
            .prefix("launcher")
            .suffix(".zip")
            .tempfile()
            .map_err(|e| runner2::Error::Other(format!("Failed to create temporary file: {}", e)))?;
        let download_path = temp_file.path().to_path_buf();
        
        let sender_clone = sender.clone();
        network.download_file(&content.url, &download_path, move |progress| {
            let percentage = if progress.total_bytes > 0 {
                progress.bytes as f32 / progress.total_bytes as f32
            } else {
                0.0
            };
            let _ = sender_clone.send(UiMessage::SetDownloadProgress {
                progress: percentage,
                speed_kbps: progress.speed_kbps,
            });
        }).await?;
        
        info!("Download complete: {}", download_path.display());

        // Extract package
        info!("Extracting launcher package");
        sender.send(UiMessage::SetStatus("Extracting launcher...".into()))
            .map_err(|e| runner2::Error::Other(e.to_string()))?;
        
        // Remove old files before extracting new ones
        info!("Removing old files");
        file_manager.remove_old_files()?;
        
        // Extract to Patcher directory in the install directory
        let extract_path = FileManager::get_patcher_dir(app_slug)?;
        file_manager.extract_zip(&download_path, &extract_path)?;
        info!("Extraction complete: {}", extract_path.display());

        // Save the current version
        info!("Saving version information");
        file_manager.save_version(&version, &patcher_secret)?;
        info!("Version {} saved", version);

        // Clean up the temporary file
        if let Err(e) = temp_file.close() {
            warn!("Failed to remove temporary file: {}", e);
            // Non-critical error, continue execution
        }

        // Launch the new version
        launch_from_manifest(&extract_path, &file_manager, &launcher_data, &launcher, &sender)?;
    } else {
        warn!("No content URLs found");
    }

    info!("Runner completed successfully");
    Ok(())
}

fn launch_from_manifest(
    extract_path: &std::path::Path,
    file_manager: &FileManager,
    launcher_data: &LauncherData,
    launcher: &Launcher,
    sender: &Sender<UiMessage>,
) -> Result<()> {
    // Read manifest
    info!("Reading manifest file {}", extract_path.join("patcher.manifest").display());
    let manifest_path = extract_path.join("patcher.manifest");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| {
            error!("Failed to read manifest: {}", e);
            runner2::Error::Manifest(format!("Failed to read manifest: {}", e))
        })?;
    let mut manifest = ManifestManager::new(&manifest_content)?;
    info!("Successfully read manifest");

    // Set up manifest variables
    info!("Setting up manifest variables");
    manifest.set_variable("exedir", extract_path.to_string_lossy().into());
    manifest.set_variable("installdir", file_manager.get_install_dir().to_string_lossy().into());
    let encoded_secret = config::secret::encode_secret(&launcher_data.app_secret);
    manifest.set_variable("secret", encoded_secret);
    manifest.set_variable("lockfile", "launcher.lock".into());
    manifest.set_variable("network-status", "online".into());

    // Launch the executable
    info!("Launching executable");
    sender.send(UiMessage::SetStatus("Launching...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let target = manifest.get_target()?;
    let arguments = manifest.get_arguments()?;
    info!("Launching {} with arguments: {:?}", target.display(), arguments);
    launcher.launch_executable(target, &arguments)?;
    info!("Launcher started successfully");

    sender.send(UiMessage::SetProgress(1.0))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    sender.send(UiMessage::Close)
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::fs;
    use tempfile::TempDir;
    use std::io::Write;
    use log::LevelFilter;

    #[test]
    fn test_message_sending() {
        let (tx, rx) = channel();
        tx.send(UiMessage::SetProgress(0.5)).unwrap();
        assert!(matches!(rx.recv().unwrap(), UiMessage::SetProgress(0.5)));
    }

    #[test]
    fn test_log_file_creation() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("launcher-log.txt");

        // Try to create and write to the log file
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap();

        // Set up logging with a custom writer that flushes after each write
        struct FlushingWriter<W: Write>(W);
        impl<W: Write> Write for FlushingWriter<W> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let result = self.0.write(buf);
                self.0.flush()?;
                result
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.flush()
            }
        }

        let flushing_writer = FlushingWriter(log_file);

        // Set up logging
        let mut builder = env_logger::Builder::new();
        builder.format_timestamp_millis();
        builder.filter_level(LevelFilter::Info);
        builder.target(env_logger::Target::Pipe(Box::new(flushing_writer)));
        builder.init();

        // Write some log messages
        log::info!("Test log message 1");
        log::error!("Test error message");
        log::info!("Test log message 2");

        // Read the log file contents
        let contents = fs::read_to_string(&log_path).unwrap();

        // Verify log messages were written
        assert!(contents.contains("Test log message 1"), "Log file contents: {}", contents);
        assert!(contents.contains("Test error message"), "Log file contents: {}", contents);
        assert!(contents.contains("Test log message 2"), "Log file contents: {}", contents);
    }
}
