use crate::Result;
use std::path::Path;
use std::process::Command;
use log::info;

pub struct Launcher;

impl Launcher {
    pub fn new() -> Self {
        Self
    }

    pub fn launch_executable<P: AsRef<Path>>(&self, executable: P, arguments: &[String]) -> Result<()> {
        let executable = executable.as_ref();
        info!("Launching executable: {:?}", executable);
        let absolute_path = if executable.is_absolute() {
            executable.to_path_buf()
        } else {
            let current_path = std::env::current_dir()?.join(executable);
            if current_path.exists() {
                current_path
            } else {
                which::which(executable)?
            }
        };
        
        if cfg!(target_os = "macos") && absolute_path.extension().map_or(false, |ext| ext == "app") {
            // For macOS .app bundles, we need to use the 'open' command
            let mut cmd = Command::new("/usr/bin/open");
            
            // Convert the path to a string, keeping it relative if it was relative
            let app_path = if executable.is_absolute() {
                executable.to_string_lossy().to_string()
            } else {
                executable.to_string_lossy().to_string()
            };
            
            cmd.arg(&app_path);
            
            if !arguments.is_empty() {
                cmd.arg("--args");
                cmd.args(arguments);
            }
            
            info!("Launching /usr/bin/open with arguments: {:?}", cmd.get_args().collect::<Vec<_>>());
            cmd.spawn()?.wait()?;
        } else {
            // For regular executables, run them directly
            let mut cmd = Command::new(&absolute_path);
            cmd.args(arguments);
            
            // Get the current executable's directory
            let exe_path = std::env::current_exe()?;
            let current_dir = exe_path.parent().ok_or_else(|| {
                crate::Error::Other("Failed to get parent directory of the current executable".into())
            })?;
            
            info!("Setting current directory to {}", current_dir.display());
            cmd.current_dir(current_dir);
            
            info!("Launching {} with arguments: {:?}", absolute_path.display(), arguments);
            
            if cfg!(target_os = "windows") {
                // On Windows, just spawn and don't wait
                cmd.spawn()?;
            } else {
                // On other platforms, wait for completion as before
                let status = cmd.spawn()?.wait()?;
                if !status.success() {
                    return Err(crate::Error::Other(format!(
                        "Launcher exited with status: {}",
                        status
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_executable() {
        let launcher = Launcher::new();
        let echo = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "echo"
        };
        
        let args = if cfg!(target_os = "windows") {
            vec!["/C".to_string(), "echo".to_string(), "test".to_string()]
        } else {
            vec!["test".to_string()]
        };

        assert!(launcher.launch_executable(echo, &args).is_ok());
    }
} 