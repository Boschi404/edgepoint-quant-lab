use qs_core::*;
use qs_storage::{AtomicCheckpointStore, RunCheckpoint, StorageLayout};
use std::collections::BTreeMap;

#[test]
fn checkpoint_roundtrip_verifies_checksum() {
    let root = std::env::temp_dir().join(format!("qs_checkpoint_test_{}", std::process::id()));
    let run_id = RunId("run_test".into());
    let store = AtomicCheckpointStore::new(StorageLayout::new(&root));
    let checkpoint = RunCheckpoint {
        schema_version: 1,
        run_id: run_id.clone(),
        run_state: PersistentRunState::Running,
        completed_components: vec![],
        component_states: BTreeMap::new(),
        search_state: None,
        partial_results_index: serde_json::json!({}),
        ranking_state: serde_json::json!({}),
        rng_state: serde_json::json!({"seed": 1}),
        metadata: RunMetadata::default(),
        created_at: 1,
        checksum: String::new(),
    };
    if let Err(err) = store.save_latest(&checkpoint) {
        panic!("save checkpoint: {err}");
    }
    let loaded = match store.load_latest(&run_id) {
        Ok(value) => value,
        Err(err) => panic!("load checkpoint: {err}"),
    };
    assert_eq!(loaded.run_id.0, run_id.0);
    let _ = std::fs::remove_dir_all(root);
}
