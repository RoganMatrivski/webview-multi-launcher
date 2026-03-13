use color_eyre::Report;
use config::{Config, File};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

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
}

fn main() -> Result<(), Report> {
    let args = init::initialize()?;

    let cfg = if let Some(c) = args.config {
        c.to_string_lossy().to_string()
    } else {
        "config".into()
    };

    let cfg = Config::builder()
        // TODO: Probably add appdata folder?
        .add_source(File::with_name(&cfg))
        .build()?;

    let profiles: AppConfig = cfg.try_deserialize()?;
    let profiles = profiles.profiles;
    let keys = profiles.keys().collect::<Vec<_>>();
    let choices = profiles
        .iter()
        .map(|(n, p)| format!("{n} ({})", p.to_string_lossy()))
        .collect::<Vec<_>>();

    let profile = {
        use dialoguer::Select;

        let selection = Select::new()
            .with_prompt("Pick the profile")
            .items(&choices)
            .default(0)
            .interact()?;

        profiles[keys[selection]].clone()
    };

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    gtk::init().expect("failed to init gtk");

    std::fs::create_dir_all(&profile)?;

    let mut web_context = WebContext::new(Some(profile));

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Profile WebView")
        .with_maximized(true)
        .build(&event_loop)?;

    let builder =
        WebViewBuilder::new_with_web_context(&mut web_context).with_url("https://mail.zoho.com");

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
