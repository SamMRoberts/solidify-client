#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
use solidify_client::{application::Application, storage::ProfileService};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::Manager;

#[derive(Default)]
struct Shutdown {
    started: AtomicBool,
    done: AtomicBool,
}
fn shutdown(app: &tauri::AppHandle) {
    if app.state::<Shutdown>().started.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let connections = app.state::<Arc<Application>>();
        let profiles = app.state::<Arc<ProfileService>>();
        tokio::join!(connections.shutdown(), profiles.shutdown());
        app.state::<Shutdown>().done.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}
fn main() {
    let app = tauri::Builder::default()
        .manage(Arc::new(Application::new()))
        .manage(Shutdown::default())
        .setup(|app| {
            app.manage(Arc::new(ProfileService::new(
                app.path().app_config_dir().ok(),
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_connection,
            commands::poll_connection,
            commands::send_line,
            commands::update_viewport,
            commands::disconnect,
            commands::profiles::load_profiles,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::update_profile_appearance,
            commands::profiles::delete_profile,
            commands::profiles::select_profile
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && !window.state::<Shutdown>().done.load(Ordering::SeqCst)
            {
                api.prevent_close();
                shutdown(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("could not initialize the desktop application");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !app.state::<Shutdown>().done.load(Ordering::SeqCst)
        {
            api.prevent_exit();
            shutdown(app);
        }
    });
}
