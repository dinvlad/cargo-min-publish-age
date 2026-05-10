use std::path::PathBuf;
use std::process::Command;

pub fn install(port: u16, age: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let home = std::env::var("HOME")?;
    let label = "io.github.cargo-min-publish-age";
    let launch_agents_dir = PathBuf::from(&home).join("Library/LaunchAgents");
    let plist_path = launch_agents_dir.join(format!("{}.plist", label));
    let plist_content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
             <key>Label</key>\n\
             <string>{}</string>\n\
             <key>ProgramArguments</key>\n\
             <array>\n\
                 <string>{}</string>\n\
                 <string>proxy</string>\n\
                 <string>--port</string>\n\
                 <string>{}</string>\n\
             </array>\n\
             <key>RunAtLoad</key>\n\
             <true/>\n\
             <key>KeepAlive</key>\n\
             <true/>\n\
         </dict>\n\
         </plist>\n",
        label,
        exe.display(),
        port
    );
    std::fs::write(&plist_path, plist_content)?;

    let uid = get_uid()?;
    let _ = Command::new("launchctl")
        .args([
            "bootout",
            &format!("gui/{}", uid),
            plist_path.to_str().unwrap(),
        ])
        .status();

    let bootstrap = Command::new("launchctl")
        .args([
            "bootstrap",
            &format!("gui/{}", uid),
            plist_path.to_str().unwrap(),
        ])
        .status()?;
    if !bootstrap.success() {
        return Err("launchctl bootstrap failed".into());
    }

    crate::config::write_install_config(port, age)?;

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    crate::config::remove_install_config()?;

    let label = "io.github.cargo-min-publish-age";
    let home = std::env::var("HOME").unwrap_or_default();
    let plist_path = PathBuf::from(&home).join(format!("Library/LaunchAgents/{}.plist", label));

    if plist_path.exists() {
        let uid = get_uid()?;
        let _ = Command::new("launchctl")
            .args([
                "bootout",
                &format!("gui/{}", uid),
                plist_path.to_str().unwrap(),
            ])
            .status();
        let _ = std::fs::remove_file(&plist_path);
    }

    Ok(())
}

fn get_uid() -> Result<u32, Box<dyn std::error::Error>> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() {
        return Err("id -u failed".into());
    }
    let uid_str = String::from_utf8_lossy(&output.stdout);
    Ok(uid_str.trim().parse()?)
}
