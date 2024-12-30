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

const WINDOW_WIDTH: f32 = 400.0;
const WINDOW_HEIGHT: f32 = 200.0;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Starting PatchKit Runner");

    let options = eframe::NativeOptions {
        default_theme: eframe::Theme::Dark,
        viewport: ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_resizable(false),
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
    info!("Reading test.dat file");
    let dat_file = std::fs::File::open("test.dat")
        .map_err(|e| {
            error!("Failed to open test.dat: {}", e);
            runner2::Error::DatFile(format!("Failed to open test.dat: {}", e))
        })?;
    let launcher_data = LauncherData::from_binary(dat_file)?;
    info!("Successfully read test.dat");
    
    // Initialize file manager with the first 8 chars of app secret
    let app_slug = &launcher_data.app_secret[..8];
    let mut file_manager = FileManager::new(app_slug)?;
    let launcher = Launcher::new();

    // Check network connection
    info!("Checking network connection");
    sender.send(UiMessage::SetStatus("Checking network connection...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;

    if !network.check_connection().await? {
        return Err(runner2::Error::Other("No internet connection".into()));
    }
    info!("Network connection established");

    // Get latest version
    info!("Fetching latest version");
    sender.send(UiMessage::SetStatus("Fetching latest version...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let version = network.get_latest_version(&launcher_data.patcher_secret).await?;
    info!("Latest version: {}", version);

    // Get download URLs
    info!("Getting download URLs");
    sender.send(UiMessage::SetStatus("Getting download URLs...".into()))
        .map_err(|e| runner2::Error::Other(e.to_string()))?;
    let content_urls = network
        .get_content_urls(&launcher_data.patcher_secret, &version)
        .await?;

    if let Some(content) = content_urls.first() {
        info!("Found content URL: {}", content.url);
        
        // Download launcher package
        info!("Downloading launcher package");
        sender.send(UiMessage::SetStatus("Downloading launcher...".into()))
            .map_err(|e| runner2::Error::Other(e.to_string()))?;
        let download_path = PathBuf::from("launcher.zip");
        
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
        let extract_path = PathBuf::from("launcher");
        file_manager.extract_zip(&download_path, &extract_path)?;
        info!("Extraction complete: {}", extract_path.display());

        // Read manifest
        info!("Reading manifest file");
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
    } else {
        warn!("No content URLs found");
    }

    info!("Runner completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn test_message_sending() {
        let (tx, rx) = channel();
        tx.send(UiMessage::SetProgress(0.5)).unwrap();
        assert!(matches!(rx.recv().unwrap(), UiMessage::SetProgress(0.5)));
    }
}
