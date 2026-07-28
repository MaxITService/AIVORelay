// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{error::ErrorKind, Parser};

fn main() {
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/dri").exists()
            && std::env::var("WAYLAND_DISPLAY").is_err()
            && std::env::var("XDG_SESSION_TYPE").unwrap_or_default() == "x11"
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let raw_arguments = std::env::args_os().collect::<Vec<_>>();
    let cli_args = match aivorelay_app_lib::CliArgs::try_parse_from(&raw_arguments) {
        Ok(arguments) => arguments,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.exit()
        }
        Err(error)
            if raw_arguments
                .iter()
                .any(|argument| argument.to_string_lossy() == "--json") =>
        {
            println!(
                "{}",
                serde_json::json!({
                    "ok": false,
                    "operation": "cli_parse",
                    "error": error.to_string(),
                    "exit_code": 2,
                })
            );
            std::process::exit(2);
        }
        Err(error) => error.exit(),
    };

    aivorelay_app_lib::run(cli_args)
}
