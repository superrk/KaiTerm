mod models;
mod error;
mod ssh_session;
mod sftp_ops;
mod transfers;
mod local_term;
mod crypto;
mod sys_info;

use ssh_session::SessionManager;
use transfers::TransferManager;
use local_term::LocalManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
    .filter_module("russh", log::LevelFilter::Warn)
    .init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionManager::new())
        .manage(TransferManager::new())
        .manage(LocalManager::new())
        .invoke_handler(tauri::generate_handler![
            // SSH
            ssh_session::connect_ssh,
            ssh_session::disconnect_ssh,
            ssh_session::reconnect_ssh,
            ssh_session::write_stdin,
            ssh_session::resize_terminal,
            // SFTP
            sftp_ops::sftp_list_dir,
            sftp_ops::sftp_mkdir,
            sftp_ops::sftp_rename,
            sftp_ops::sftp_remove,
            sftp_ops::sftp_read_file,
            sftp_ops::sftp_write_file,
            // Transfer
            transfers::upload_file,
            transfers::download_file,
            transfers::cancel_transfer,
            transfers::resolve_transfer_conflict,
            // Local
            local_term::detect_shells,
            local_term::start_local_shell,
            local_term::write_local_stdin,
            local_term::stop_local_shell,
            local_term::list_local_files,
            local_term::resize_local_terminal,
            local_term::open_in_explorer,
            // Connections
            crypto::save_connection,
            crypto::duplicate_connection,
            crypto::load_connections,
            crypto::get_credential,
            crypto::delete_connection,
            crypto::rename_group,
            crypto::load_groups,
            crypto::save_groups,
            // Host key
            ssh_session::trust_host_key,
            ssh_session::cancel_host_key,
            ssh_session::get_known_hosts,
            ssh_session::remove_known_host,
            // System info
            sys_info::sysinfo_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KaiTerm");
}
