use std::path::PathBuf;
use std::process::Command;

pub fn install(port: u16, age: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;

    if !has_systemd() {
        return Err("systemd not detected; install requires systemd user services".into());
    }

    let home = std::env::var("HOME")?;
    let service_dir = PathBuf::from(&home).join(".config/systemd/user");
    if !service_dir.exists() {
        std::fs::create_dir_all(&service_dir)?;
    }

    let service_path = service_dir.join("cargo-min-publish-age.service");
    let service_content = format!(
        "[Unit]\n\
         Description=cargo-min-publish-age registry proxy\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={} proxy --port {}\n\
         Restart=on-failure\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe.display(),
        port
    );
    std::fs::write(&service_path, service_content)?;

    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !reload.success() {
        return Err("systemctl --user daemon-reload failed".into());
    }

    let enable = Command::new("systemctl")
        .args(["--user", "enable", "--now", "cargo-min-publish-age"])
        .status()?;
    if !enable.success() {
        return Err("systemctl --user enable --now cargo-min-publish-age failed".into());
    }

    crate::config::write_install_config(port, age)?;

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    crate::config::remove_install_config()?;

    let _ = Command::new("systemctl")
        .args(["--user", "stop", "cargo-min-publish-age"])
        .status();
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "cargo-min-publish-age"])
        .status();

    let home = std::env::var("HOME").unwrap_or_default();
    let service_path =
        PathBuf::from(&home).join(".config/systemd/user/cargo-min-publish-age.service");
    if service_path.exists() {
        let _ = std::fs::remove_file(&service_path);
    }

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    Ok(())
}

fn has_systemd() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
