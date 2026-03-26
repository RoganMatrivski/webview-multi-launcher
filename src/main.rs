use color_eyre::{
    Report,
    eyre::{self, ContextCompat},
};
use config::{Config, File};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{WebContext, WebViewBuilder};

mod init;

#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Deserialize)]
struct AppConfig {
    profiles: HashMap<String, PathBuf>,
    url: url::Url,
}

fn main() -> Result<(), Report> {
    let args = init::initialize()?;

    let projdir =
        directories::ProjectDirs::from("com.github", "roganmatrivski", "webview-multi-launcher")
            .wrap_err("Failed to setup project file")?;
    let appdata_dir = projdir.data_dir();
    std::fs::create_dir_all(&appdata_dir)?;

    let appdata_config = appdata_dir.join("config.toml");

    if !appdata_config.exists() {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Define default configuration content based on AppConfig structure
        let default_config_content = include_str!("default_profile.toml");

        // Open the file, create if it doesn't exist, and truncate it if it does
        // This ensures we write fresh default content when the file is created.
        let mut _file = OpenOptions::new()
            .create(true) // create if it doesn't exist
            .write(true) // open for writing
            .truncate(true) // clear existing content if file already exists (though !exists() check should prevent this)
            .open(&appdata_config)?;

        // Write the default content to the file
        _file.write_all(default_config_content.as_bytes())?;

        // Update the modification time
        #[cfg(unix)]
        {
            let now = filetime::FileTime::from_system_time(SystemTime::now());
            filetime::set_file_times(&appdata_config, now, now).unwrap();
        }
    }

    tracing::trace!(p = args.profile, "profile");
    if args.profile {
        return open_path(&appdata_config);
    }

    let appdata_config = appdata_config.to_string_lossy().to_string();

    let cfg = if let Some(c) = args.config {
        c.to_string_lossy().to_string()
    } else {
        "config".into()
    };

    let cfg = Config::builder()
        // TODO: Probably add appdata folder?
        .add_source(File::with_name(&cfg).required(false))
        .add_source(File::with_name(&appdata_config))
        .build()?;

    let configs: AppConfig = cfg.try_deserialize()?;
    let profiles = configs.profiles;
    let keys = profiles.keys().collect::<Vec<_>>();
    let choices = profiles
        .iter()
        .map(|(n, p)| format!("{n} ({})", p.to_string_lossy()))
        .collect::<Vec<_>>();

    let profile = {
        use dialoguer::FuzzySelect;

        let selection = FuzzySelect::new()
            .with_prompt("Pick the profile")
            .items(&choices)
            .default(0)
            .interact()?;

        appdata_dir
            .join("profiles")
            .join(profiles[keys[selection]].clone())
    };

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    gtk::init().expect("failed to init gtk");

    tracing::trace!(p = ?profile.to_string_lossy(), "Creating profile dir");
    std::fs::create_dir_all(&profile)?;

    let mut web_context = WebContext::new(Some(profile));

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("WebView")
        .with_maximized(true)
        .build(&event_loop)?;

    let builder = WebViewBuilder::new_with_web_context(&mut web_context).with_url(configs.url);

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = builder.build(&window)?;

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        builder.build_gtk(vbox)?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn open_path(p: &Path) -> eyre::Result<()> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        // On Windows, use `cmd /C start` to open the file

        use color_eyre::eyre::Context;
        let status = Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("") // Empty title for the new window, important for `start` command syntax
            .arg(p)
            .status()
            .wrap_err_with(|| format!("Failed to execute `cmd /C start` for path: {:?}", p))?;

        if status.success() {
            println!("Opened file successfully!");
        } else {
            // Using eyre::bail! for better error reporting with color_eyre
            eyre::bail!("Failed to open file on Windows with status: {:?}", status);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows systems (e.g., Linux), use `xdg-open`
        let status = Command::new("xdg-open")
            .arg(p)
            .status()
            .wrap_err_with(|| format!("Failed to execute `xdg-open` for path: {:?}", p))?;

        if status.success() {
            println!("Opened file successfully!");
        } else {
            // Using eyre::bail! for better error reporting with color_eyre
            eyre::bail!(
                "Failed to open file on Unix-like system with status: {:?}",
                status
            );
        }
    }

    Ok(())
}
