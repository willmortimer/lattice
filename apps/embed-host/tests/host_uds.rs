use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use lattice_embed_host::{
    install_model, run_server, socket_path_in, BackendKind, EmbedHostClient, HostConfig, HostState,
};
use lattice_embedding::{
    sha256_hex, EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, ModelManifest,
    PoolingStrategy, MANIFEST_SCHEMA_VERSION,
};
use tempfile::tempdir;
use tokio::process::Command;
use tokio::time::sleep;

fn write_fixture_model(dir: &std::path::Path) -> (PathBuf, PathBuf, String) {
    let artifact_bytes = b"fake-qwen3-fixture-bytes";
    let sha = sha256_hex(artifact_bytes);
    let artifact = dir.join("fixture.bin");
    fs::write(&artifact, artifact_bytes).unwrap();

    let manifest = ModelManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        provider: "fake".into(),
        model_id: "Qwen/Qwen3-Embedding-0.6B-GGUF".into(),
        model_revision: "test-rev".into(),
        artifact: "fixture.bin".into(),
        sha256: sha.clone(),
        license: "Apache-2.0".into(),
        native_dimensions: 32,
        default_dimensions: 8,
        pooling: PoolingStrategy::Last,
        instruction_version: "lattice-retrieval-v1".into(),
    };
    let manifest_path = dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    (manifest_path, artifact, sha)
}

async fn wait_for_socket(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            if EmbedHostClient::connect(path).await.is_ok() {
                return;
            }
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("socket not ready: {}", path.display());
}

#[tokio::test]
async fn fake_backend_embeds_over_uds() {
    let dir = tempdir().unwrap();
    let socket = socket_path_in(dir.path());
    let models_dir = dir.path().join("models");
    let (manifest_path, artifact_path, _) = write_fixture_model(dir.path());

    let installed = install_model(&manifest_path, &artifact_path, &models_dir).unwrap();

    let state = HostState::new(HostConfig::new(
        socket.clone(),
        BackendKind::Fake,
        models_dir.clone(),
    ));
    let server = tokio::spawn(run_server(Arc::clone(&state)));

    wait_for_socket(&socket).await;

    let client = Arc::new(EmbedHostClient::connect(&socket).await.unwrap());
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.backend, "fake");

    let session = client
        .load_model(&installed.model_dir, Some(8))
        .await
        .unwrap();
    assert_eq!(session.specification().dimensions, 8);

    let query = session
        .embed_query(EmbedQueryRequest {
            text: "hello lattice".into(),
        })
        .await
        .unwrap();
    assert_eq!(query.values.len(), 8);

    let docs = session
        .embed_documents(vec![
            EmbedDocumentRequest {
                chunk_id: "c1".into(),
                text: "alpha".into(),
            },
            EmbedDocumentRequest {
                chunk_id: "c2".into(),
                text: "beta".into(),
            },
        ])
        .await
        .unwrap();
    assert_eq!(docs.len(), 2);
    assert_ne!(docs[0].values, docs[1].values);

    let status = client.status().await.unwrap();
    assert_eq!(status.install_state, "ready");
    assert_eq!(status.queries_completed, 1);
    assert_eq!(status.documents_completed, 2);

    client.unload_model().await.unwrap();
    let status = client.status().await.unwrap();
    assert_eq!(status.install_state, "not-installed");

    server.abort();
}

#[tokio::test]
async fn client_tolerates_host_crash() {
    let dir = tempdir().unwrap();
    let socket = socket_path_in(dir.path());
    let models_dir = dir.path().join("models");
    let (manifest_path, artifact_path, _) = write_fixture_model(dir.path());
    let installed = install_model(&manifest_path, &artifact_path, &models_dir).unwrap();

    let bin = env!("CARGO_BIN_EXE_lattice-embed-host");
    let mut child = Command::new(bin)
        .arg("serve")
        .arg("--socket")
        .arg(&socket)
        .arg("--backend")
        .arg("fake")
        .arg("--models-dir")
        .arg(&models_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn embed-host");

    wait_for_socket(&socket).await;

    let client = Arc::new(EmbedHostClient::connect(&socket).await.unwrap());
    let session = client
        .load_model(&installed.model_dir, None)
        .await
        .unwrap();
    let _ = session
        .embed_query(EmbedQueryRequest {
            text: "before crash".into(),
        })
        .await
        .unwrap();

    child.kill().await.expect("kill host");
    let _ = child.wait().await;

    // Give the kernel a moment to tear down the socket endpoint.
    sleep(Duration::from_millis(50)).await;

    let err = session
        .embed_query(EmbedQueryRequest {
            text: "after crash".into(),
        })
        .await
        .expect_err("host should be gone");
    let message = err.to_string();
    assert!(
        message.contains("closed")
            || message.contains("Connection")
            || message.contains("Broken pipe")
            || message.contains("No such file")
            || message.contains("os error")
            || message.contains("provider error"),
        "unexpected error after crash: {message}"
    );
}

#[tokio::test]
async fn install_rpc_via_client() {
    let dir = tempdir().unwrap();
    let socket = socket_path_in(dir.path());
    let models_dir = dir.path().join("models");
    let (manifest_path, artifact_path, sha) = write_fixture_model(dir.path());

    let state = HostState::new(HostConfig::new(
        socket.clone(),
        BackendKind::Fake,
        models_dir.clone(),
    ));
    let server = tokio::spawn(run_server(state));
    wait_for_socket(&socket).await;

    let client = EmbedHostClient::connect(&socket).await.unwrap();
    let installed = client
        .install_model(&manifest_path, &artifact_path, &models_dir)
        .await
        .unwrap();
    assert_eq!(installed.artifact_sha256, sha);
    assert!(PathBuf::from(&installed.model_dir).join("fixture.bin").is_file());

    server.abort();
}
