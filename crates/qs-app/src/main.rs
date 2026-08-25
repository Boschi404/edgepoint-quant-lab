mod component_factory;
mod runtime_launcher;

use qs_api::{router, ApiState, RunManager};
use qs_storage::{AtomicCheckpointStore, RecoveryService, RunCatalog, StorageLayout};
use runtime_launcher::AppRunLauncher;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() {
    let bind = match std::env::var("QS_BIND") {
        Ok(value) => value,
        Err(_) => "0.0.0.0:8080".to_owned(),
    };
    let addr: SocketAddr = match bind.parse() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("invalid QS_BIND value {bind}: {err}");
            std::process::exit(2);
        }
    };

    let boot_registry = match component_factory::build_static_strategy_registry() {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("failed to build strategy registry: {err}");
            std::process::exit(2);
        }
    };
    let component_count = component_factory::build_default_components(boot_registry.clone()).len();
    println!("registered {component_count} pipeline components");
    println!(
        "registered {} static strategy plugins",
        boot_registry.list_ids().len()
    );

    let storage_root = match std::env::var("QS_RUNS_ROOT") {
        Ok(value) => PathBuf::from(value),
        Err(_) => PathBuf::from("./runs"),
    };
    run_startup_recovery(&storage_root);
    let runs = RunManager::default();
    let launcher = AppRunLauncher::new(runs.clone(), storage_root.clone());
    let app = router(ApiState::new(runs, Some(Arc::new(launcher)), storage_root));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("failed to bind {addr}: {err}");
            std::process::exit(2);
        }
    };

    println!("quant-system API listening on http://{addr}");
    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("server error: {err}");
        std::process::exit(1);
    }
}

fn run_startup_recovery(storage_root: &PathBuf) {
    let catalog_path = storage_root.join("catalog").join("runs.sqlite");
    let catalog = match RunCatalog::open(&catalog_path) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("startup recovery: catalog unavailable: {err}");
            return;
        }
    };
    let layout = StorageLayout::new(storage_root.clone());
    let recovery = RecoveryService {
        checkpoint_store: AtomicCheckpointStore::new(layout),
    };
    match recovery.mark_interrupted_on_startup(&catalog) {
        Ok(count) if count > 0 => {
            println!("startup recovery: marked {count} running runs as interrupted")
        }
        Ok(_) => (),
        Err(err) => eprintln!("startup recovery failed: {err}"),
    }
}
