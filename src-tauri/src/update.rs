//! Updates from the GitHub releases (tauri-plugin-updater).
//!
//! Half a minute after launch, and then every six hours, NeedleX downloads
//! `latest.json` from the latest GitHub release. Nothing about you or your
//! music is sent. When the release is newer than the running app, the menu-bar
//! menu offers it at the top. Choosing it downloads the update, checks its
//! signature against the public key in tauri.conf.json, swaps the app bundle
//! and restarts NeedleX. "Check for Updates…" looks right away.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::AppState;

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Default, PartialEq)]
pub enum Status {
    #[default]
    Unknown,
    Checking,
    UpToDate,
    Failed,
    /// this version can be installed
    Available(String),
    Installing(String),
}

#[derive(Default)]
pub struct UpdateState(Mutex<Status>);

pub fn status(app: &AppHandle) -> Status {
    app.state::<AppState>().update.0.lock().unwrap().clone()
}

fn set(app: &AppHandle, s: Status) {
    let state = app.state::<AppState>();
    let changed = {
        let mut cur = state.update.0.lock().unwrap();
        let changed = *cur != s;
        *cur = s;
        changed
    };
    if changed {
        crate::rebuild_menu(app);
    }
}

pub fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// The line under the version in the menu: what the updater knows.
pub fn menu_status(app: &AppHandle) -> String {
    let v = current_version(app);
    match status(app) {
        Status::Unknown => format!("NeedleX {v}"),
        Status::Checking => format!("NeedleX {v} – checking for updates…"),
        Status::UpToDate => format!("NeedleX {v} – up to date"),
        Status::Failed => format!("NeedleX {v} – couldn't reach the update server"),
        Status::Available(new) => format!("NeedleX {v} – {new} is available"),
        Status::Installing(new) => format!("NeedleX {v} – installing {new}…"),
    }
}

/// Checks now and then for the whole time NeedleX runs.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check(&app, false).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// "Check for Updates…" in the menu.
pub fn check_now(app: &AppHandle) {
    let a = app.clone();
    tauri::async_runtime::spawn(async move { check(&a, true).await });
}

async fn check(app: &AppHandle, manual: bool) {
    let before = status(app);
    if matches!(before, Status::Checking | Status::Installing(_)) {
        return;
    }
    if manual {
        set(app, Status::Checking);
    }
    let found = match app.updater() {
        Ok(u) => u.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    let next = match found {
        Ok(Some(u)) => Status::Available(u.version),
        Ok(None) => Status::UpToDate,
        Err(e) => {
            eprintln!("NeedleX: update check failed: {e}");
            // a background check that fails (offline) keeps what we knew
            if manual {
                Status::Failed
            } else {
                before
            }
        }
    };
    set(app, next);
}

/// Downloads and installs the available update, then restarts NeedleX.
pub fn install(app: &AppHandle) {
    let Status::Available(version) = status(app) else { return };
    set(app, Status::Installing(version));
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let update = a.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
            let Some(update) = update else { return Err("the update is gone".to_string()) };
            update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())
        }
        .await;
        match result {
            Ok(()) => a.restart(),
            Err(e) => {
                eprintln!("NeedleX: update failed: {e}");
                set(&a, Status::Failed);
            }
        }
    });
}
