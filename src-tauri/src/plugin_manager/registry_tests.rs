use super::*;
use crate::plugin_manager::models::{InputsSpec, TargetDescriptorDto};
use std::path::PathBuf;

fn test_app_state(
    registry: PluginRegistry,
    state_path: Option<PathBuf>,
    library_root: PathBuf,
) -> crate::AppState {
    crate::AppState {
        plugins: std::sync::Arc::new(tokio::sync::RwLock::new(registry)),
        jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::plugin_manager::executor::ActiveJobTracker::new(),
        )),
        state_path,
        db: std::sync::Arc::new(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        )),
        library_root: std::sync::Arc::new(library_root),
    }
}

fn mock_manifest(id: &str) -> PluginManifest {
    PluginManifest {
        schema: None,
        manifest_version: 1,
        metadata: crate::plugin_manager::models::PluginMetadata {
            id: id.to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            license: None,
            description: "Test".to_string(),
            category: None,
            tags: vec![],
            min_core_version: None,
        },
        runtime: crate::plugin_manager::models::PluginRuntime {
            runtime_type: "python".to_string(),
            entrypoint: "main.py".to_string(),
            min_python_version: None,
            dependencies_file: None,
            healthcheck_flag: None,
        },
        parameters: "parameters.json".to_string(),
        inputs: "inputs.json".to_string(),
        outputs: "outputs.json".to_string(),
        execution: None,
    }
}

fn mock_bundle(id: &str) -> PluginBundle {
    PluginBundle {
        install_path: PathBuf::from("/mock/plugins").join(id),
        manifest: PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: id.to_string(),
                name: format!("Plugin {}", id),
                version: "1.0.0".to_string(),
                author: Some("Author".to_string()),
                license: Some("MIT".to_string()),
                description: "Description".to_string(),
                category: Some("Vegetation".to_string()),
                tags: vec!["vegetation".to_string()],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: Some(crate::plugin_manager::models::PluginExecutionConfig {
                timeout_seconds: 60,
                provides_output_dir_arg: true,
                concurrency: "sequential".to_string(),
                gpu: None,
            }),
        },
        inputs_spec: InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string(), "image/tiff".to_string()],
            requires_metadata: Vec::new(),
            optional_metadata: Vec::new(),
            supports_aoi: true,
            aoi_format: None,
        },
        inputs_raw: serde_json::json!({
            "granularity": "single_image",
            "supported_mime_types": ["image/jpeg", "image/tiff"],
            "supports_aoi": true
        }),
        parameters_raw: serde_json::json!({}),
        outputs_raw: serde_json::json!({}),
        enabled: true,
        source: "builtin".to_string(),
        health_status: "healthy".to_string(),
        custom_parameters: serde_json::json!({}),
    }
}

#[tokio::test]
async fn test_app_state_has_plugins_and_concurrent_access() {
    let state = test_app_state(PluginRegistry::new(), None, PathBuf::from("/tmp"));

    {
        let mut reg = state.plugins.write().await;
        reg.register(mock_bundle("rgb-exg"));
    }

    let snapshot = {
        let reg = state.plugins.read().await;
        assert_eq!(reg.plugins.len(), 1);
        reg.clone()
    };
    assert_eq!(snapshot.get_summaries().len(), 1);
}

#[test]
fn test_validate_subcontracts_valid() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();

    std::fs::write(
        plugin_dir.join("parameters.json"),
        r#"{"parameters": [{"name": "thresh", "label": "Threshold", "type": "number", "default": 0.5}]}"#,
    )
    .unwrap();

    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();

    std::fs::write(
        plugin_dir.join("outputs.json"),
        r#"{"metrics": [], "artifacts": []}"#,
    )
    .unwrap();

    let manifest = mock_manifest("test-plugin");

    let res = validate_subcontracts(plugin_dir, &manifest);
    assert!(res.is_ok());
    let (params, inputs_spec, outputs) = res.unwrap();
    assert!(params.get("parameters").is_some());
    assert_eq!(inputs_spec.granularity, "single_image");
    assert!(outputs.get("metrics").is_some());
}

#[test]
fn test_validate_subcontracts_missing_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();

    // only create inputs.json, omitting parameters.json and outputs.json
    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();

    let manifest = mock_manifest("test-plugin");

    let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
    assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
}

#[test]
fn test_validate_subcontracts_empty_mime_types() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();

    std::fs::write(plugin_dir.join("parameters.json"), r#"{"parameters": []}"#).unwrap();

    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": []}"#,
    )
    .unwrap();

    std::fs::write(plugin_dir.join("outputs.json"), r#"{"metrics": []}"#).unwrap();

    let manifest = mock_manifest("test-plugin");

    let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
    assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
}

#[test]
fn test_discover_plugins_success_and_fault_tolerance() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugins_root = temp_dir.path();

    // 1. Valid plugin A
    let dir_a = plugins_root.join("plugin-a");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::write(
        dir_a.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-a", "name": "Plugin A", "version": "1.0.0", "description": "A"},
            "runtime": {"type": "python", "entrypoint": "main.py"},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    std::fs::write(
        dir_a.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("outputs.json"), r#"{}"#).unwrap();

    // 2. Corrupt plugin B (invalid manifest JSON)
    let dir_b = plugins_root.join("plugin-b");
    std::fs::create_dir_all(&dir_b).unwrap();
    std::fs::write(dir_b.join("manifest.json"), "{ broken").unwrap();

    // 3. Plugin C (missing subcontracts)
    let dir_c = plugins_root.join("plugin-c");
    std::fs::create_dir_all(&dir_c).unwrap();
    std::fs::write(
        dir_c.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-c", "name": "Plugin C", "version": "1.0.0", "description": "C"},
            "runtime": {"type": "python", "entrypoint": "main.py"},
            "parameters": "missing_params.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();

    // 4. Valid plugin D
    let dir_d = plugins_root.join("plugin-d");
    std::fs::create_dir_all(&dir_d).unwrap();
    std::fs::write(
        dir_d.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-d", "name": "Plugin D", "version": "2.0.0", "description": "D"},
            "runtime": {"type": "binary", "entrypoint": "detector"},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();
    std::fs::write(dir_d.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    std::fs::write(
        dir_d.join("inputs.json"),
        r#"{"granularity": "batch", "supported_mime_types": ["image/tiff"]}"#,
    )
    .unwrap();
    std::fs::write(dir_d.join("outputs.json"), r#"{}"#).unwrap();

    let discovered = discover_plugins(plugins_root).expect("discover_plugins should succeed");
    assert_eq!(
        discovered.len(),
        2,
        "Fault-tolerant discovery must skip corrupted plugins"
    );
    let ids: Vec<&str> = discovered
        .iter()
        .map(|b| b.manifest.metadata.id.as_str())
        .collect();
    assert_eq!(ids, vec!["plugin-a", "plugin-d"]);
}

#[test]
fn test_discover_plugins_nonexistent_dir() {
    let fake_path = Path::new("/path/that/does/not/exist/definitely_not");
    let res = discover_plugins(fake_path);
    assert!(res.is_ok());
    assert_eq!(res.unwrap().len(), 0);
}

#[tokio::test]
async fn test_check_plugin_health_internal_missing_plugin() {
    let app_state = test_app_state(PluginRegistry::new(), None, PathBuf::from("/tmp"));

    let err = check_plugin_health_internal("missing-plugin", &app_state)
        .await
        .unwrap_err();
    assert!(matches!(err, PluginError::NotFound(_)));
}

#[tokio::test]
async fn test_execute_healthcheck_python_healthy() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();
    let script_path = plugin_dir.join("main.py");
    std::fs::write(
        &script_path,
        r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy", "python_version": "3.10"}))
    sys.exit(0)
sys.exit(1)
"#,
    )
    .unwrap();

    let mut bundle = mock_bundle("test-health");
    bundle.install_path = plugin_dir.to_path_buf();
    bundle.manifest.runtime.entrypoint = "main.py".to_string();

    let status = execute_healthcheck(&bundle).await;
    assert_eq!(status.status, "healthy");
    assert_eq!(status.plugin_id, "test-health");
    assert!(status.details.is_some());
    assert!(status.error.is_none());
}

#[tokio::test]
async fn test_execute_healthcheck_python_unhealthy() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();
    let script_path = plugin_dir.join("main.py");
    std::fs::write(
        &script_path,
        r#"
import sys
if "--healthcheck" in sys.argv:
    sys.stderr.write("Missing OpenCV dependency\n")
    sys.exit(1)
sys.exit(1)
"#,
    )
    .unwrap();

    let mut bundle = mock_bundle("test-unhealthy");
    bundle.install_path = plugin_dir.to_path_buf();
    bundle.manifest.runtime.entrypoint = "main.py".to_string();

    let status = execute_healthcheck(&bundle).await;
    assert_eq!(status.status, "unhealthy");
    assert!(status.error.is_some());
    assert!(status.error.unwrap().contains("Missing OpenCV"));
}

#[tokio::test]
async fn test_execute_healthcheck_missing_entrypoint() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut bundle = mock_bundle("test-missing-entry");
    bundle.install_path = temp_dir.path().to_path_buf();
    bundle.manifest.runtime.entrypoint = "nonexistent.py".to_string();

    let status = execute_healthcheck(&bundle).await;
    assert_eq!(status.status, "unhealthy");
    assert!(status.error.is_some());
}

#[tokio::test]
async fn test_check_plugin_health_updates_in_memory_bundle_status() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();
    let script_path = plugin_dir.join("main.py");
    std::fs::write(
        &script_path,
        r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy"}))
    sys.exit(0)
"#,
    )
    .unwrap();

    let mut bundle = mock_bundle("test-update-health");
    bundle.install_path = plugin_dir.to_path_buf();
    bundle.manifest.runtime.entrypoint = "main.py".to_string();
    bundle.health_status = "unknown".to_string();

    let mut registry = PluginRegistry::new();
    registry.register(bundle);

    let app_state = test_app_state(registry, None, PathBuf::from("/tmp"));

    let res = check_plugin_health_internal("test-update-health", &app_state).await;
    assert!(res.is_ok());
    assert_eq!(res.unwrap().status, "healthy");

    let reg = app_state.plugins.read().await;
    assert_eq!(
        reg.plugins.get("test-update-health").unwrap().health_status,
        "healthy"
    );
}

#[tokio::test]
async fn test_execute_healthcheck_parses_json_error_on_success_status() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();
    let script_path = plugin_dir.join("main.py");
    std::fs::write(
        &script_path,
        r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "unhealthy", "error": "Model weights missing"}))
    sys.exit(0)
"#,
    )
    .unwrap();

    let mut bundle = mock_bundle("test-unhealthy-zero-exit");
    bundle.install_path = plugin_dir.to_path_buf();
    bundle.manifest.runtime.entrypoint = "main.py".to_string();

    let status = execute_healthcheck(&bundle).await;
    assert_eq!(status.status, "unhealthy");
    assert_eq!(status.error.as_deref(), Some("Model weights missing"));
}

#[tokio::test]
async fn test_check_plugin_health_flushes_to_state_store() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();
    let script_path = plugin_dir.join("main.py");
    std::fs::write(
        &script_path,
        r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy"}))
    sys.exit(0)
"#,
    )
    .unwrap();

    let mut bundle = mock_bundle("test-health-persist");
    bundle.install_path = plugin_dir.to_path_buf();
    bundle.manifest.runtime.entrypoint = "main.py".to_string();

    let mut registry = PluginRegistry::new();
    registry.register(bundle);

    let state_path = temp_dir.path().join("plugin_state.json");
    let app_state = test_app_state(registry, Some(state_path.clone()), PathBuf::from("/tmp"));

    let res = check_plugin_health_internal("test-health-persist", &app_state).await;
    assert!(res.is_ok());

    assert!(
        state_path.exists(),
        "State file must exist after health check"
    );
    let store = crate::plugin_manager::lifecycle::load_plugin_state(&state_path).unwrap();
    let record = store
        .plugins
        .get("test-health-persist")
        .expect("record should exist");
    let hc = record
        .last_healthcheck
        .as_ref()
        .expect("last_healthcheck should be set");
    assert_eq!(hc.status, "healthy");
}

#[test]
fn test_validate_subcontracts_enforces_parameters_schema() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();

    // Parameter missing required 'type' and 'default'
    std::fs::write(
        plugin_dir.join("parameters.json"),
        r#"{"parameters": [{"name": "thresh", "label": "Threshold"}]}"#,
    )
    .unwrap();
    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(plugin_dir.join("outputs.json"), r#"{}"#).unwrap();

    let manifest = mock_manifest("test-plugin");

    let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
    assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
}

#[test]
fn test_validate_subcontracts_enforces_inputs_granularity_and_mime() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path();

    std::fs::write(plugin_dir.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    // Invalid granularity
    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "invalid_granularity", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(plugin_dir.join("outputs.json"), r#"{}"#).unwrap();

    let manifest = mock_manifest("test-plugin");

    let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
    assert!(matches!(err, PluginError::SubcontractInvalid { .. }));

    // Invalid MIME type
    std::fs::write(
        plugin_dir.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["text/plain"]}"#,
    )
    .unwrap();
    let err_mime = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
    assert!(matches!(err_mime, PluginError::SubcontractInvalid { .. }));
}

#[test]
fn test_rescan_plugins_discovers_new_plugin_on_disk() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugins_root = temp_dir.path();

    // 1. Initially create plugin A
    let dir_a = plugins_root.join("plugin-a");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::write(
        dir_a.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-a", "name": "Plugin A", "version": "1.0.0", "description": "A"},
            "runtime": {"type": "python", "entrypoint": "main.py"},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    std::fs::write(
        dir_a.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("outputs.json"), r#"{}"#).unwrap();

    let state_path = plugins_root.join("plugin_state.json");
    let app_state = test_app_state(
        crate::init_plugin_registry_from_dir(plugins_root),
        Some(state_path),
        temp_dir.path().join("library"),
    );

    // Initially only plugin A
    let initial_reg = rescan_plugins_internal(&app_state);
    assert_eq!(initial_reg.plugins.len(), 1);
    assert!(initial_reg.plugins.contains_key("plugin-a"));

    // 2. Add plugin B to disk while app is running
    let dir_b = plugins_root.join("plugin-b");
    std::fs::create_dir_all(&dir_b).unwrap();
    std::fs::write(
        dir_b.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-b", "name": "Plugin B", "version": "1.0.0", "description": "B"},
            "runtime": {"type": "python", "entrypoint": "main.py"},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();
    std::fs::write(dir_b.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    std::fs::write(
        dir_b.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(dir_b.join("outputs.json"), r#"{}"#).unwrap();

    // 3. Rescan discovers plugin B
    let rescanned = rescan_plugins_internal(&app_state);
    assert_eq!(rescanned.plugins.len(), 2);
    assert!(rescanned.plugins.contains_key("plugin-a"));
    assert!(rescanned.plugins.contains_key("plugin-b"));
}

#[tokio::test]
async fn test_get_installed_plugins_internal_returns_summaries() {
    let mut registry = PluginRegistry::new();
    registry.register(mock_bundle("rgb-exg"));
    registry.register(mock_bundle("tree-count"));

    let app_state = test_app_state(registry, None, PathBuf::from("/tmp"));

    let mut summaries = get_installed_plugins_internal(&app_state).await.unwrap();
    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id, "rgb-exg");
    assert_eq!(summaries[1].id, "tree-count");
}

#[tokio::test]
async fn test_query_compatible_plugins_internal_filters_targets() {
    let mut registry = PluginRegistry::new();
    let mut jpeg_only = mock_bundle("jpeg-only");
    jpeg_only.inputs_spec.supported_mime_types = vec!["image/jpeg".to_string()];
    registry.register(jpeg_only);

    let mut tiff_only = mock_bundle("tiff-only");
    tiff_only.inputs_spec.supported_mime_types = vec!["image/tiff".to_string()];
    registry.register(tiff_only);

    let app_state = test_app_state(registry, None, PathBuf::from("/tmp"));

    let target = TargetDescriptorDto {
        granularity: "single_image".to_string(),
        mime_type: "image/jpeg".to_string(),
        has_gps: true,
        has_altitude: true,
        has_aoi: false,
        has_resolution: true,
    };

    let matches = query_compatible_plugins_internal(target, &app_state)
        .await
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "jpeg-only");
}

#[tokio::test]
async fn test_reload_plugins_internal_updates_registry_and_returns_summaries() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugins_root = temp_dir.path();

    let dir_a = plugins_root.join("plugin-a");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::write(
        dir_a.join("manifest.json"),
        r#"{
            "manifest_version": 1,
            "metadata": {"id": "plugin-a", "name": "Plugin A", "version": "1.0.0", "description": "A"},
            "runtime": {"type": "python", "entrypoint": "main.py"},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
    std::fs::write(
        dir_a.join("inputs.json"),
        r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
    )
    .unwrap();
    std::fs::write(dir_a.join("outputs.json"), r#"{}"#).unwrap();

    let app_state = test_app_state(
        PluginRegistry::new(),
        Some(plugins_root.join("plugin_state.json")),
        temp_dir.path().join("library"),
    );

    let summaries = reload_plugins_internal(&app_state).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "plugin-a");

    let reg = app_state.plugins.read().await;
    assert!(reg.plugins.contains_key("plugin-a"));
}
