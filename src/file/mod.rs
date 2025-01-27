use crate::Result;
use directories::BaseDirs;
use std::fs::{self, File};
use std::io::{self, Write, Read, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use zip::ZipArchive;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
use log::{debug, info, warn};

pub struct FileManager {
    install_dir: PathBuf,
    installed_files: Vec<PathBuf>,
    secret_slug: String,
}

#[derive(Debug)]
pub struct VersionInfo {
    pub version: String,
    pub patcher_secret: String,
}

impl VersionInfo {
    pub fn new(version: String, patcher_secret: String) -> Self {
        Self {
            version,
            patcher_secret,
        }
    }

    pub fn from_string(content: &str) -> Option<Self> {
        let parts: Vec<&str> = content.trim().split(':').collect();
        if parts.len() == 2 {
            Some(Self {
                patcher_secret: parts[0].to_string(),
                version: parts[1].to_string(),
            })
        } else {
            None
        }
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.patcher_secret, self.version)
    }
}

impl FileManager {
    pub fn get_patcher_dir(secret_slug: &str) -> Result<PathBuf> {
        if cfg!(target_os = "macos") {
            let base_dirs = BaseDirs::new()
                .ok_or_else(|| crate::Error::FileSystem("Could not determine base directories".into()))?;
            
            Ok(base_dirs
                .data_dir()
                .join("PatchKit")
                .join("Apps")
                .join(secret_slug)
                .join("Patcher"))
        } else {
            // Get the directory where the runner executable is located
            let exe_path = std::env::current_exe()?;
            let runner_dir = exe_path.parent().ok_or_else(|| {
                crate::Error::FileSystem("Failed to get parent directory of the current executable".into())
            })?;
            
            Ok(runner_dir.join("Patcher"))
        }
    }

    pub fn new(secret_slug: &str) -> Result<Self> {
        let install_dir = if cfg!(target_os = "macos") {
            let base_dirs = BaseDirs::new()
                .ok_or_else(|| crate::Error::FileSystem("Could not determine base directories".into()))?;
            
            base_dirs
                .data_dir()
                .join("PatchKit")
                .join("Apps")
                .join(secret_slug)
                .join("Data")
        } else {
            // For Windows and Linux, use current directory and create app directory
            std::env::current_exe()?
                .parent()
                .ok_or_else(|| crate::Error::FileSystem("Failed to get parent directory of the current executable".into()))?
                .join("app")
        };

        let mut manager = Self {
            install_dir,
            installed_files: Vec::new(),
            secret_slug: secret_slug.to_string(),
        };
        
        // Try to load the list of installed files, but it's fine if it doesn't exist
        if let Err(e) = manager.load_installed_files() {
            debug!("Failed to load installed files list: {}", e);
        }

        Ok(manager)
    }

    fn get_installed_files_path(&self) -> PathBuf {
        Self::get_patcher_dir(&self.secret_slug).unwrap().join("installed_files.txt")
    }

    fn load_installed_files(&mut self) -> Result<()> {
        let path = self.get_installed_files_path();
        if !path.exists() {
            debug!("No installed files list found at {}", path.display());
            return Ok(());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        self.installed_files.clear();
        
        let patcher_dir = Self::get_patcher_dir(&self.secret_slug)?;
        for line in io::BufRead::lines(reader) {
            let line = line?;
            self.installed_files.push(patcher_dir.join(line));
        }
        
        debug!("Loaded {} installed files", self.installed_files.len());
        Ok(())
    }

    fn save_installed_files(&self) -> Result<()> {
        let path = self.get_installed_files_path();
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        
        let patcher_dir = Self::get_patcher_dir(&self.secret_slug)?;
        for path in &self.installed_files {
            if let Ok(relative) = path.strip_prefix(&patcher_dir) {
                writeln!(writer, "{}", relative.to_string_lossy())?;
            } else {
                warn!("Failed to make path relative: {}", path.display());
            }
        }
        
        debug!("Saved {} installed files", self.installed_files.len());
        Ok(())
    }

    pub fn get_install_dir(&self) -> &Path {
        &self.install_dir
    }

    pub fn create_install_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.install_dir)?;
        Ok(())
    }

    pub fn get_current_version(&self) -> Result<Option<VersionInfo>> {
        let version_file = Self::get_patcher_dir(&self.secret_slug)?.join("version.txt");
        debug!("Checking version file: {}", version_file.display());
        
        if !version_file.exists() {
            debug!("Version file does not exist");
            return Ok(None);
        }

        let mut content = String::new();
        File::open(version_file)?.read_to_string(&mut content)?;
        debug!("Read version file content: {}", content);
        
        // Try to parse as new format first
        if let Some(version_info) = VersionInfo::from_string(&content) {
            debug!("Successfully parsed version info: {:?}", version_info);
            return Ok(Some(version_info));
        }
        
        // If parsing failed, treat it as old format (version only)
        // In this case, we return None to force redownload
        debug!("Version file in old format, will force redownload");
        Ok(None)
    }

    pub fn save_version(&self, version: &str, patcher_secret: &str) -> Result<()> {
        let version_info = VersionInfo::new(version.to_string(), patcher_secret.to_string());
        let version_file = Self::get_patcher_dir(&self.secret_slug)?.join("version.txt");
        debug!("Saving version to file: {}", version_file.display());
        
        // Make sure the Patcher directory exists
        if let Some(parent) = version_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(version_file)?;
        let content = version_info.to_string();
        debug!("Writing version content: {}", content);
        write!(file, "{}", content)?;
        Ok(())
    }

    pub fn needs_update(&self, new_version: &str, new_patcher_secret: &str) -> Result<bool> {
        match self.get_current_version()? {
            Some(current_version) => Ok(
                current_version.version != new_version || 
                current_version.patcher_secret != new_patcher_secret
            ),
            None => Ok(true)
        }
    }

    pub fn extract_zip<P: AsRef<Path>>(&mut self, zip_path: P, destination: P) -> Result<()> {
        let file = File::open(&zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        // Clear the installed files list before new extraction
        self.installed_files.clear();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = destination.as_ref().join(file.mangled_name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;

                #[cfg(target_os = "macos")]
                {
                    // Check if the file is in Contents/MacOS directory
                    if outpath.to_string_lossy().contains("Contents/MacOS") {
                        // Set executable permissions (read/write/execute for owner, read/execute for group and others)
                        let perms = fs::Permissions::from_mode(0o755);
                        fs::set_permissions(&outpath, perms)?;
                    }
                }
            }

            debug!("Extracted: {}", outpath.display());
            self.installed_files.push(outpath);
        }

        // Save the list of installed files
        self.save_installed_files()?;

        Ok(())
    }

    pub fn remove_old_files(&self) -> Result<()> {
        if self.installed_files.is_empty() {
            debug!("No list of installed files, skipping cleanup");
            return Ok(());
        }

        info!("Removing {} old files", self.installed_files.len());
        for path in self.installed_files.iter().rev() {
            if path.is_file() {
                if let Err(e) = fs::remove_file(path) {
                    warn!("Failed to remove file {}: {}", path.display(), e);
                } else {
                    debug!("Removed file: {}", path.display());
                }
            } else if path.is_dir() {
                // Only remove directory if it's empty
                if fs::read_dir(path)?.next().is_none() {
                    if let Err(e) = fs::remove_dir(path) {
                        warn!("Failed to remove directory {}: {}", path.display(), e);
                    } else {
                        debug!("Removed directory: {}", path.display());
                    }
                } else {
                    debug!("Skipping non-empty directory: {}", path.display());
                }
            }
        }
        Ok(())
    }

    pub fn create_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        write!(file, "{}", std::process::id())?;
        Ok(())
    }

    pub fn check_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Ok(false);
        }

        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = SystemTime::now().duration_since(modified) {
                    if duration > Duration::from_secs(60) {
                        fs::remove_file(path)?;
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    pub fn delete_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_create_install_dir() {
        let manager = FileManager::new("test123").unwrap();
        assert!(manager.create_install_dir().is_ok());
        assert!(manager.get_install_dir().exists());
        fs::remove_dir_all(manager.get_install_dir()).unwrap_or(());
    }

    #[test]
    fn test_lockfile_operations() {
        let manager = FileManager::new("test123").unwrap();
        let temp_dir = tempdir().unwrap();
        let lockfile_path = temp_dir.path().join("test.lock");

        // Create lockfile
        assert!(manager.create_lockfile(&lockfile_path).is_ok());
        assert!(lockfile_path.exists());

        // Check lockfile
        assert!(manager.check_lockfile(&lockfile_path).unwrap());

        // Delete lockfile
        assert!(manager.delete_lockfile(&lockfile_path).is_ok());
        assert!(!lockfile_path.exists());
    }

    #[test]
    fn test_extract_zip() {
        let temp_dir = tempdir().unwrap();
        let secret_slug = "test123";
        
        // Mock the current executable path for testing
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let mut manager = FileManager::new(secret_slug).unwrap();
        manager.install_dir = temp_dir.path().join("app");
        manager.create_install_dir().unwrap();
        
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = FileManager::get_patcher_dir(secret_slug).unwrap();
        fs::create_dir_all(&extract_dir).unwrap();

        // Create a test zip file
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        zip.start_file("test.txt", zip::write::FileOptions::default()).unwrap();
        zip.write_all(b"test content").unwrap();
        zip.finish().unwrap();

        // Extract the zip file
        assert!(manager.extract_zip(&zip_path, &extract_dir).is_ok());
        assert!(extract_dir.join("test.txt").exists());
    }

    #[test]
    fn test_version_management() {
        let temp_dir = tempdir().unwrap();
        let secret_slug = "test123";
        
        debug!("Test directory: {}", temp_dir.path().display());
        
        // Mock the current executable path for testing
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Make sure the Patcher directory doesn't exist
        let patcher_dir = FileManager::get_patcher_dir(secret_slug).unwrap();
        if patcher_dir.exists() {
            fs::remove_dir_all(&patcher_dir).unwrap();
        }
        debug!("Patcher dir: {}", patcher_dir.display());
        
        let mut manager = FileManager::new(secret_slug).unwrap();
        manager.install_dir = temp_dir.path().join("app");
        manager.create_install_dir().unwrap();
        
        debug!("Install dir: {}", manager.install_dir.display());

        // Initially there should be no version
        let version_result = manager.get_current_version().unwrap();
        debug!("Initial version: {:?}", version_result);
        assert!(version_result.is_none());

        // Save version and verify it
        let test_version = "1.0.0";
        let test_secret = "test_secret";
        manager.save_version(test_version, test_secret).unwrap();
        
        let current = manager.get_current_version().unwrap().unwrap();
        debug!("Current version after save: {:?}", current);
        assert_eq!(current.version, test_version);
        assert_eq!(current.patcher_secret, test_secret);

        // Check if update is needed - same version, same secret
        assert!(!manager.needs_update(test_version, test_secret).unwrap());
        
        // Check if update is needed - different version, same secret
        assert!(manager.needs_update("2.0.0", test_secret).unwrap());
        
        // Check if update is needed - same version, different secret
        assert!(manager.needs_update(test_version, "new_secret").unwrap());
        
        // Check if update is needed - different version, different secret
        assert!(manager.needs_update("2.0.0", "new_secret").unwrap());
    }

    #[test]
    fn test_version_info_parsing() {
        // Test valid format
        let info = VersionInfo::from_string("secret123:1.0.0").unwrap();
        assert_eq!(info.patcher_secret, "secret123");
        assert_eq!(info.version, "1.0.0");
        
        // Test invalid format
        assert!(VersionInfo::from_string("invalid_format").is_none());
        assert!(VersionInfo::from_string("too:many:parts").is_none());
        
        // Test to_string
        let info = VersionInfo::new("1.0.0".to_string(), "secret123".to_string());
        assert_eq!(info.to_string(), "secret123:1.0.0");
    }

    #[test]
    fn test_file_cleanup() {
        let temp_dir = tempdir().unwrap();
        let secret_slug = "test123";
        
        // Mock the current executable path for testing
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let mut manager = FileManager::new(secret_slug).unwrap();
        manager.install_dir = temp_dir.path().join("app");
        manager.create_install_dir().unwrap();
        
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = FileManager::get_patcher_dir(secret_slug).unwrap();
        fs::create_dir_all(&extract_dir).unwrap();

        // Create a test zip file with multiple files and directories
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        
        // Add a file in a directory
        zip.add_directory("test_dir", Default::default()).unwrap();
        zip.start_file("test_dir/test1.txt", Default::default()).unwrap();
        zip.write_all(b"test content 1").unwrap();
        
        // Add another file
        zip.start_file("test2.txt", Default::default()).unwrap();
        zip.write_all(b"test content 2").unwrap();
        
        zip.finish().unwrap();

        // Extract the zip file
        manager.extract_zip(&zip_path, &extract_dir).unwrap();

        // Verify files were extracted
        assert!(extract_dir.join("test_dir").join("test1.txt").exists());
        assert!(extract_dir.join("test2.txt").exists());

        // Remove old files
        manager.remove_old_files().unwrap();

        // Verify files were removed
        assert!(!extract_dir.join("test_dir").join("test1.txt").exists());
        assert!(!extract_dir.join("test2.txt").exists());
        // Directory should be removed as it's empty
        assert!(!extract_dir.join("test_dir").exists());
    }

    #[test]
    fn test_installed_files_persistence() {
        let temp_dir = tempdir().unwrap();
        let secret_slug = "test123";
        
        debug!("Test directory: {}", temp_dir.path().display());
        
        // Mock the current executable path for testing
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Create first instance and extract files
        {
            let mut manager = FileManager::new(secret_slug).unwrap();
            manager.install_dir = temp_dir.path().join("app"); // Override install dir for testing
            manager.create_install_dir().unwrap();
            
            debug!("Install dir: {}", manager.install_dir.display());
            
            // Create and extract a test zip
            let zip_path = temp_dir.path().join("test.zip");
            let extract_dir = FileManager::get_patcher_dir(secret_slug).unwrap();
            fs::create_dir_all(&extract_dir).unwrap();
            
            debug!("Extract dir: {}", extract_dir.display());
            
            let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
            
            zip.add_directory("test_dir", Default::default()).unwrap();
            zip.start_file("test_dir/test1.txt", Default::default()).unwrap();
            zip.write_all(b"test content 1").unwrap();
            zip.start_file("test2.txt", Default::default()).unwrap();
            zip.write_all(b"test content 2").unwrap();
            zip.finish().unwrap();

            // Extract and verify files are saved
            manager.extract_zip(&zip_path, &extract_dir).unwrap();
            let installed_files_path = manager.get_installed_files_path();
            debug!("Installed files path: {}", installed_files_path.display());
            assert!(installed_files_path.exists());
        }

        // Create second instance and verify files are loaded
        {
            let mut manager = FileManager::new(secret_slug).unwrap();
            manager.install_dir = temp_dir.path().join("app");
            
            debug!("Second instance install dir: {}", manager.install_dir.display());
            debug!("Second instance installed files path: {}", manager.get_installed_files_path().display());
            
            // Load installed files explicitly since we're using a custom install_dir
            manager.load_installed_files().unwrap();
            
            debug!("Loaded {} installed files", manager.installed_files.len());
            for file in &manager.installed_files {
                debug!("Found installed file: {}", file.display());
            }
            
            assert!(!manager.installed_files.is_empty());
            assert!(manager.installed_files.iter().any(|p| p.file_name().unwrap().to_str().unwrap() == "test2.txt"));
            assert!(manager.installed_files.iter().any(|p| p.file_name().unwrap().to_str().unwrap() == "test1.txt"));
            
            // Remove files and verify they're gone
            manager.remove_old_files().unwrap();
            for path in &manager.installed_files {
                assert!(!path.exists());
            }
        }
    }
} 