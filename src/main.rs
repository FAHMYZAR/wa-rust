mod config;
mod core;
mod features;
mod utils;

use crate::core::dispatcher::Dispatcher;
use crate::utils::qr;
use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    crate::utils::system::init_start_time();

    // Every scratch file lives in `disk/tmp`. The sweeper runs on a five-minute
    // cycle anchored to `disk/.cleanup-stamp`, so the schedule carries over
    // across restarts instead of resetting on every boot.
    crate::utils::media::spawn_temp_cleanup_task();

    let config = Arc::new(config::Config::from_env()?);

    let registry = features::build_registry();
    let dispatcher = Arc::new(Dispatcher {
        registry: Arc::clone(&registry),
        config: Arc::clone(&config),
    });

    let args: Vec<String> = std::env::args().collect();
    let phone_number = parse_arg(&args, "--phone", "-p").or_else(|| {
        std::env::var("PAIR_PHONE")
            .ok()
            .filter(|phone| !phone.trim().is_empty())
    });

    let store = SqliteStore::new("whatsapp.db").await?;
    info!("SQLite backend initialized");

    let mut builder = Bot::builder()
        .with_backend(store)
        .on_qr_code(|code, timeout| async move {
            info!(
                "QR code (valid {}s) — scan via WhatsApp > Linked Devices:",
                timeout.as_secs()
            );
            match qr::render_ascii(&code) {
                Ok(ascii) => println!("\n{ascii}\n"),
                Err(e) => error!("QR render failed ({e}); raw payload: {code}"),
            }
        })
        .on_pair_code(|code, timeout| async move {
            info!(
                "Pair code (valid {}s): enter this on your phone:",
                timeout.as_secs()
            );
            info!(">>> {code} <<<");
        })
        .on_connected(|_client| async {
            info!("Connected successfully!");
        })
        .on_logged_out(|_info| async {
            error!("Logged out — delete whatsapp.db to re-pair");
        })
        .on_message(move |ctx| {
            let dispatcher = Arc::clone(&dispatcher);
            async move {
                if !ctx.info.push_name.is_empty() {
                    crate::utils::contact::record_contact(
                        &ctx.info.source.sender,
                        &ctx.info.push_name,
                    );
                    if !ctx.info.source.is_group {
                        crate::utils::contact::record_contact(
                            &ctx.info.source.chat,
                            &ctx.info.push_name,
                        );
                    }
                }
                if let Err(e) = dispatcher.handle(&ctx).await {
                    error!("dispatch failed: {e:#}");
                }
            }
        });

    if let Some(phone) = phone_number {
        // Pair code uses the same pairing flow regardless of prefix choice;
        // the user must still link via Linked Devices on their phone.
        info!("Pair code requested; offering QR and pair code concurrently");
        builder = builder.with_pair_code(whatsapp_rust::pair_code::PairCodeOptions {
            phone_number: phone,
            ..Default::default()
        });
    }
    let mut handle = builder.build().await?.spawn();

    tokio::select! {
        _ = &mut handle => {}
        _ = whatsapp_rust::shutdown_signal() => {
            info!("Shutdown signal received");
            handle.shutdown().await;
        }
    }

    Ok(())
}

fn parse_arg(args: &[String], long: &str, short: &str) -> Option<String> {
    let long_prefix = format!("{long}=");
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == long || arg == short {
            return iter.next().cloned();
        }
        if let Some(value) = arg.strip_prefix(&long_prefix) {
            return Some(value.to_string());
        }
    }
    None
}
