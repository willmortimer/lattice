//! celld Connect/HTTP client + projection run loop.

use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::Value;

use crate::config::{celld_base_url, require_celld_base_url};
use crate::connect::{
    decode_connect_stream, decode_unary_json, encode_connect_message, encode_unary_json,
    CELL_APPLY, CELL_START, CONNECT_PROTOCOL_VERSION, GUEST_INVOKE,
};
use crate::error::{CellClientError, Result};
use crate::hydrate::{
    cell_spec_network_attachments, cell_spec_volume_attachments, hydrate_files_under_role,
    oci_suppresses_network_deny_all, KernelFSHydrationPlan, KernelFSRole,
};
use crate::types::{
    ApplyCellRequest, ApplyCellResponse, CellSpec, CollectOutputRequest, CollectOutputResponse,
    HydrateFile, HydrateProjectionRequest, HydrateProjectionResponse, ProfileRef, ResourceSpec,
    RunTaskRequest, RunTaskResponse, StartCellRequest, StartCellResponse,
};

/// Default guest services advertised for Lattice agent loops.
pub const DEFAULT_ADVERTISE_SERVICES: &[&str] = &[
    "cell.control.v1",
    "cell.exec.v1",
    "lattice.runtime.v1",
    "cell.mirror.v1",
];

/// Lattice runtime named service for hydrate / run / collect.
pub const LATTICE_RUNTIME_V1: &str = "lattice.runtime.v1";
/// Mirror broker (CollectOutput alias also works on lattice.runtime.v1).
pub const CELL_MIRROR_V1: &str = "cell.mirror.v1";

/// One collected output artifact keyed by projection-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub content: Vec<u8>,
}

/// Path → output file map returned by the run loop.
pub type OutputFileMap = BTreeMap<String, OutputFile>;

/// High-level Apply→Start→Hydrate→Run→Collect request.
#[derive(Debug, Clone)]
pub struct ProjectionRunRequest {
    pub cell_id: String,
    pub projection_id: String,
    pub profile: String,
    pub plan: KernelFSHydrationPlan,
    /// When empty, files are loaded from `plan.input` host dirs under role `input/`.
    pub hydrate_files: Vec<HydrateFile>,
    pub argv: Vec<String>,
    pub task_id: String,
    pub timeout_sec: Option<u64>,
    /// When true (default), RunTask skips inline collect and CollectOutput is used.
    pub collect_via_mirror: bool,
    pub idempotency_key: String,
    pub resources: ResourceSpec,
    pub advertise_services: Vec<String>,
    pub allow_recreate: bool,
    /// Proto enum name, e.g. [`crate::EXECUTION_MODE_OCI`]. Empty = microVM / celld backend default.
    pub execution_mode: String,
    pub oci_bundle_path: String,
}

impl Default for ProjectionRunRequest {
    fn default() -> Self {
        Self {
            cell_id: String::new(),
            projection_id: String::new(),
            profile: "lattice-runtime".to_string(),
            plan: KernelFSHydrationPlan::default(),
            hydrate_files: Vec::new(),
            argv: Vec::new(),
            task_id: String::new(),
            timeout_sec: Some(60),
            collect_via_mirror: true,
            idempotency_key: String::new(),
            resources: ResourceSpec::new(1, 256 << 20),
            advertise_services: DEFAULT_ADVERTISE_SERVICES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            allow_recreate: false,
            execution_mode: String::new(),
            oci_bundle_path: String::new(),
        }
    }
}

/// Result of [`CelldClient::run_projection`].
#[derive(Debug, Clone)]
pub struct ProjectionRunResult {
    pub cell_id: String,
    pub projection_id: String,
    pub apply: ApplyCellResponse,
    pub start: StartCellResponse,
    pub hydrate: HydrateProjectionResponse,
    pub run: RunTaskResponse,
    pub collect: CollectOutputResponse,
    pub output_files: OutputFileMap,
}

/// Pluggable HTTP surface for Connect unary + server-stream calls.
pub trait CelldHttpClient: Send + Sync {
    fn unary_json(
        &self,
        base_url: &str,
        procedure: &str,
        body: &[u8],
    ) -> Result<(u16, Vec<u8>)>;

    fn stream_json(
        &self,
        base_url: &str,
        procedure: &str,
        body: &[u8],
    ) -> Result<(u16, Vec<u8>)>;
}

/// Production ureq Connect/HTTP transport.
#[derive(Debug, Default, Clone, Copy)]
pub struct HttpCelldClient;

impl CelldHttpClient for HttpCelldClient {
    fn unary_json(
        &self,
        base_url: &str,
        procedure: &str,
        body: &[u8],
    ) -> Result<(u16, Vec<u8>)> {
        post_connect(
            base_url,
            procedure,
            body,
            "application/json",
            "application/json",
        )
    }

    fn stream_json(
        &self,
        base_url: &str,
        procedure: &str,
        body: &[u8],
    ) -> Result<(u16, Vec<u8>)> {
        post_connect(
            base_url,
            procedure,
            body,
            "application/connect+json",
            "application/connect+json",
        )
    }
}

fn post_connect(
    base_url: &str,
    procedure: &str,
    body: &[u8],
    content_type: &str,
    accept: &str,
) -> Result<(u16, Vec<u8>)> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), procedure);
    let response = ureq::post(&url)
        .set("Content-Type", content_type)
        .set("Accept", accept)
        .set("Connect-Protocol-Version", CONNECT_PROTOCOL_VERSION)
        .send_bytes(body)
        .map_err(|err| CellClientError::Http(err.to_string()))?;
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(CellClientError::Io)?;
    Ok((status, bytes))
}

/// Default production client.
pub fn default_client() -> Result<CelldClient<HttpCelldClient>> {
    CelldClient::from_env(HttpCelldClient)
}

/// Typed celld client.
#[derive(Debug, Clone)]
pub struct CelldClient<H: CelldHttpClient> {
    base_url: String,
    http: Arc<H>,
}

impl<H: CelldHttpClient> CelldClient<H> {
    /// Construct with an explicit base URL (already trimmed).
    pub fn new(base_url: impl Into<String>, http: H) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: Arc::new(http),
        }
    }

    /// Require [`crate::CELLD_BASE_URL_ENV`] and build a client.
    pub fn from_env(http: H) -> Result<Self> {
        Ok(Self::new(require_celld_base_url()?, http))
    }

    /// Base URL in use.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `CellService.ApplyCell`.
    pub fn apply_cell(&self, request: &ApplyCellRequest) -> Result<ApplyCellResponse> {
        self.unary(CELL_APPLY, request)
    }

    /// `CellService.StartCell`.
    pub fn start_cell(&self, request: &StartCellRequest) -> Result<StartCellResponse> {
        self.unary(CELL_START, request)
    }

    /// Invoke a guest named service method; returns decoded JSON payload.
    pub fn invoke_json<T: serde::de::DeserializeOwned>(
        &self,
        cell_id: &str,
        service: &str,
        method: &str,
        payload: &impl serde::Serialize,
    ) -> Result<T> {
        let payload_bytes = serde_json::to_vec(payload)?;
        let raw = self.invoke_raw(cell_id, service, method, &payload_bytes, "application/json")?;
        Ok(serde_json::from_slice(&raw)?)
    }

    /// `lattice.runtime.v1` / `HydrateProjection`.
    pub fn hydrate_projection(
        &self,
        cell_id: &str,
        request: &HydrateProjectionRequest,
    ) -> Result<HydrateProjectionResponse> {
        self.invoke_json(cell_id, LATTICE_RUNTIME_V1, "HydrateProjection", request)
    }

    /// `lattice.runtime.v1` / `RunTask`.
    pub fn run_task(&self, cell_id: &str, request: &RunTaskRequest) -> Result<RunTaskResponse> {
        self.invoke_json(cell_id, LATTICE_RUNTIME_V1, "RunTask", request)
    }

    /// `cell.mirror.v1` / `CollectOutput` (KernelFS `/output` prefix by default).
    pub fn collect_output(
        &self,
        cell_id: &str,
        request: &CollectOutputRequest,
    ) -> Result<CollectOutputResponse> {
        self.invoke_json(cell_id, CELL_MIRROR_V1, "CollectOutput", request)
    }

    /// Build CellSpec volumes/networks from a KernelFS plan and Apply + Start.
    pub fn apply_and_start_from_plan(
        &self,
        cell_id: &str,
        profile: &str,
        plan: &KernelFSHydrationPlan,
        resources: ResourceSpec,
        advertise_services: &[String],
        idempotency_key: &str,
        allow_recreate: bool,
        execution_mode: &str,
        oci_bundle_path: &str,
    ) -> Result<(ApplyCellResponse, StartCellResponse)> {
        if oci_suppresses_network_deny_all(plan, execution_mode) {
            eprintln!(
                "lattice-cell-client: omitting networks[].egress=none for execution_mode=oci \
                 (OCI providers reject deny-all egress); use microVM for enforced deny-all or \
                 with_network_deny_all(false) when OCI egress is acceptable"
            );
        }
        let spec = CellSpec {
            id: cell_id.to_string(),
            display_name: cell_id.to_string(),
            profile: Some(ProfileRef {
                name: profile.to_string(),
                digest: String::new(),
            }),
            resources: Some(resources),
            volumes: cell_spec_volume_attachments(plan),
            networks: cell_spec_network_attachments(plan, execution_mode),
            advertise_services: advertise_services.to_vec(),
            execution_mode: execution_mode.to_string(),
            oci_bundle_path: oci_bundle_path.to_string(),
        };
        let apply = self.apply_cell(&ApplyCellRequest {
            idempotency_key: idempotency_key.to_string(),
            spec,
            expected_spec_digest: String::new(),
            allow_recreate,
        })?;
        let start = self.start_cell(&StartCellRequest {
            cell_id: cell_id.to_string(),
            idempotency_key: if idempotency_key.is_empty() {
                String::new()
            } else {
                format!("{idempotency_key}-start")
            },
        })?;
        Ok((apply, start))
    }

    /// Full guest path: plan → apply/start → hydrate → run → collect → output map.
    ///
    /// Does not require VirtioFS; hydrate/collect use the mirror projection tree.
    pub fn run_projection(&self, request: &ProjectionRunRequest) -> Result<ProjectionRunResult> {
        if request.cell_id.trim().is_empty() {
            return Err(CellClientError::InvalidPlan("cell_id is required".into()));
        }
        if request.projection_id.trim().is_empty() {
            return Err(CellClientError::InvalidPlan(
                "projection_id is required".into(),
            ));
        }
        if request.argv.is_empty() {
            return Err(CellClientError::InvalidPlan("argv is required".into()));
        }

        let hydrate_files = if request.hydrate_files.is_empty() {
            let mut files = Vec::new();
            for input in &request.plan.input {
                if input.host_path.as_os_str().is_empty() {
                    continue;
                }
                files.extend(hydrate_files_under_role(
                    KernelFSRole::Input,
                    &input.host_path,
                )?);
            }
            files
        } else {
            request.hydrate_files.clone()
        };

        let (apply, start) = self.apply_and_start_from_plan(
            &request.cell_id,
            &request.profile,
            &request.plan,
            request.resources.clone(),
            &request.advertise_services,
            &request.idempotency_key,
            request.allow_recreate,
            &request.execution_mode,
            &request.oci_bundle_path,
        )?;

        let hydrate = self.hydrate_projection(
            &request.cell_id,
            &HydrateProjectionRequest {
                projection_id: request.projection_id.clone(),
                files: hydrate_files,
            },
        )?;

        let run = self.run_task(
            &request.cell_id,
            &RunTaskRequest {
                task_id: if request.task_id.is_empty() {
                    request.projection_id.clone()
                } else {
                    request.task_id.clone()
                },
                projection_id: request.projection_id.clone(),
                argv: request.argv.clone(),
                timeout_sec: request.timeout_sec,
                collect: Some(!request.collect_via_mirror),
            },
        )?;

        if run.state != "completed" || run.exit_code != 0 {
            return Err(CellClientError::RunTaskFailed {
                state: run.state.clone(),
                exit_code: run.exit_code,
                detail: run.detail.clone(),
            });
        }

        let collect = if request.collect_via_mirror {
            self.collect_output(
                &request.cell_id,
                &CollectOutputRequest {
                    projection_id: request.projection_id.clone(),
                    prefix: "output".to_string(),
                    include_content: Some(true),
                },
            )?
        } else {
            CollectOutputResponse {
                service: LATTICE_RUNTIME_V1.to_string(),
                method: "RunTask".to_string(),
                projection_id: request.projection_id.clone(),
                state: "collected".to_string(),
                files: run.output_files.clone(),
                file_count: run.output_files.len() as u64,
                ..CollectOutputResponse::default()
            }
        };

        let output_files = collected_to_map(&collect)?;

        Ok(ProjectionRunResult {
            cell_id: request.cell_id.clone(),
            projection_id: request.projection_id.clone(),
            apply,
            start,
            hydrate,
            run,
            collect,
            output_files,
        })
    }

    fn unary<Req: serde::Serialize, Resp: serde::de::DeserializeOwned>(
        &self,
        procedure: &str,
        request: &Req,
    ) -> Result<Resp> {
        let body = encode_unary_json(request)?;
        let (status, bytes) = self.http.unary_json(&self.base_url, procedure, &body)?;
        if !(200..300).contains(&status) {
            return Err(CellClientError::Status {
                status,
                body: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }
        decode_unary_json(&bytes)
    }

    fn invoke_raw(
        &self,
        cell_id: &str,
        service: &str,
        method: &str,
        payload: &[u8],
        content_type: &str,
    ) -> Result<Vec<u8>> {
        // GuestSessionService.Invoke is server-streaming Connect: Content-Type is
        // application/connect+json, so the request body must be enveloped. Sending
        // raw JSON makes connect-go read `{"cel…` as a length prefix and fail with
        // "promised N bytes in enveloped message".
        let invoke_body = serde_json::json!({
            "cellId": cell_id,
            "service": service,
            "method": method,
            "payload": base64::engine::general_purpose::STANDARD.encode(payload),
            "contentType": content_type,
        });
        let body = encode_connect_message(&invoke_body)?;
        let (status, bytes) = self.http.stream_json(&self.base_url, GUEST_INVOKE, &body)?;
        if !(200..300).contains(&status) {
            return Err(CellClientError::Status {
                status,
                body: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }
        let messages = decode_connect_stream(&bytes)?;
        let mut out = Vec::new();
        let mut saw_done = false;
        for msg in messages {
            if let Some(err) = msg.get("errorMessage").and_then(Value::as_str) {
                if !err.is_empty() {
                    return Err(CellClientError::Invoke(err.to_string()));
                }
            }
            if let Some(payload_b64) = msg.get("payload").and_then(Value::as_str) {
                if !payload_b64.is_empty() {
                    let chunk = base64::engine::general_purpose::STANDARD
                        .decode(payload_b64)
                        .map_err(|err| {
                            CellClientError::Connect(format!("invoke payload base64: {err}"))
                        })?;
                    out.extend_from_slice(&chunk);
                }
            }
            if msg.get("done").and_then(Value::as_bool).unwrap_or(false) {
                saw_done = true;
            }
        }
        if !saw_done && out.is_empty() {
            return Err(CellClientError::Invoke(
                "invoke stream completed without payload".into(),
            ));
        }
        Ok(out)
    }
}

fn collected_to_map(collect: &CollectOutputResponse) -> Result<OutputFileMap> {
    let mut map = OutputFileMap::new();
    for file in &collect.files {
        let content = file.content_bytes()?;
        map.insert(
            file.path.clone(),
            OutputFile {
                path: file.path.clone(),
                sha256: file.sha256.clone(),
                bytes: file.bytes,
                content,
            },
        );
    }
    Ok(map)
}

/// True when [`crate::CELLD_BASE_URL_ENV`] is configured.
pub fn celld_configured() -> bool {
    celld_base_url().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::encode_connect_message;
    use crate::hydrate::{is_oci_execution_mode, EXECUTION_MODE_OCI};
    use crate::types::CollectedFile;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MockHttp {
        unary: Mutex<VecDeque<(String, Value)>>,
        stream: Mutex<VecDeque<(String, Value)>>,
    }

    impl MockHttp {
        fn new() -> Self {
            Self {
                unary: Mutex::new(VecDeque::new()),
                stream: Mutex::new(VecDeque::new()),
            }
        }

        fn push_unary(&self, procedure: &str, response: Value) {
            self.unary
                .lock()
                .unwrap()
                .push_back((procedure.to_string(), response));
        }

        fn push_invoke_payload(&self, payload: Value) {
            let payload_b64 = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&payload).unwrap());
            let frame = serde_json::json!({
                "payload": payload_b64,
                "contentType": "application/json",
                "done": true,
            });
            self.stream
                .lock()
                .unwrap()
                .push_back((GUEST_INVOKE.to_string(), frame));
        }
    }

    impl CelldHttpClient for MockHttp {
        fn unary_json(
            &self,
            _base_url: &str,
            procedure: &str,
            _body: &[u8],
        ) -> Result<(u16, Vec<u8>)> {
            let (expected, value) = self
                .unary
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| CellClientError::Http("unexpected unary call".into()))?;
            assert_eq!(expected, procedure);
            Ok((200, serde_json::to_vec(&value)?))
        }

        fn stream_json(
            &self,
            _base_url: &str,
            procedure: &str,
            body: &[u8],
        ) -> Result<(u16, Vec<u8>)> {
            // Live celld rejects unframed connect+json bodies; keep the mock honest.
            assert!(
                body.len() >= 5 && body[0] == 0,
                "Invoke request must be Connect-enveloped, got {} bytes (first={:?})",
                body.len(),
                body.first()
            );
            let req_len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
            assert_eq!(
                body.len(),
                5 + req_len,
                "Invoke request must be a single data envelope (no end-stream trailer)"
            );
            let req: Value = serde_json::from_slice(&body[5..5 + req_len])?;
            assert_eq!(req["cellId"], "cell_demo");
            assert!(req["service"].as_str().is_some());
            assert!(req["method"].as_str().is_some());

            let (expected, value) = self
                .stream
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| CellClientError::Http("unexpected stream call".into()))?;
            assert_eq!(expected, procedure);
            Ok((200, encode_connect_message(&value)?))
        }
    }

    #[test]
    fn from_env_fails_closed() {
        let previous = std::env::var(crate::CELLD_BASE_URL_ENV).ok();
        unsafe { std::env::remove_var(crate::CELLD_BASE_URL_ENV) };
        let err = CelldClient::from_env(MockHttp::new()).unwrap_err();
        assert!(matches!(err, CellClientError::MissingBaseUrl));
        unsafe {
            match previous {
                Some(v) => std::env::set_var(crate::CELLD_BASE_URL_ENV, v),
                None => std::env::remove_var(crate::CELLD_BASE_URL_ENV),
            }
        }
    }

    #[test]
    fn run_projection_mocked_loop() {
        let http = MockHttp::new();
        http.push_unary(
            CELL_APPLY,
            serde_json::json!({
                "cell": {"id": "cell_demo", "observedState": "OBSERVED_STATE_READY"},
                "operation": {"operationId": "op_apply", "state": "OPERATION_STATE_SUCCEEDED"}
            }),
        );
        http.push_unary(
            CELL_START,
            serde_json::json!({
                "operation": {"operationId": "op_start", "state": "OPERATION_STATE_SUCCEEDED"}
            }),
        );
        http.push_invoke_payload(serde_json::json!({
            "state": "hydrated",
            "file_count": 1,
            "projection_id": "proj_demo"
        }));
        http.push_invoke_payload(serde_json::json!({
            "state": "completed",
            "exit_code": 0,
            "projection_id": "proj_demo"
        }));
        let artifact = base64::engine::general_purpose::STANDARD.encode(b"hello out");
        http.push_invoke_payload(serde_json::json!({
            "state": "collected",
            "file_count": 1,
            "files": [{
                "path": "output/out.txt",
                "sha256": "abc",
                "bytes": 9,
                "content_base64": artifact
            }]
        }));

        let client = CelldClient::new("http://celld.test", http);
        let result = client
            .run_projection(&ProjectionRunRequest {
                cell_id: "cell_demo".into(),
                projection_id: "proj_demo".into(),
                plan: KernelFSHydrationPlan::from_role_paths(
                    "/tmp/in",
                    None,
                    "/tmp/out",
                ),
                hydrate_files: vec![HydrateFile::text("input/hello.txt", "hi")],
                argv: vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "cp \"$KERNELFS_INPUT/hello.txt\" \"$KERNELFS_OUTPUT/out.txt\"".into(),
                ],
                ..ProjectionRunRequest::default()
            })
            .expect("run_projection");

        assert_eq!(result.hydrate.state, "hydrated");
        assert_eq!(result.run.exit_code, 0);
        let out = result.output_files.get("output/out.txt").unwrap();
        assert_eq!(out.content, b"hello out");
    }

    #[test]
    fn collected_map_decodes_bytes() {
        let collect = CollectOutputResponse {
            files: vec![CollectedFile {
                path: "output/a.bin".into(),
                sha256: "x".into(),
                bytes: 3,
                content_base64: base64::engine::general_purpose::STANDARD.encode([1, 2, 3]),
            }],
            ..CollectOutputResponse::default()
        };
        let map = collected_to_map(&collect).unwrap();
        assert_eq!(map["output/a.bin"].content, vec![1, 2, 3]);
    }

    #[test]
    fn projection_run_defaults_keep_lattice_runtime_for_oci() {
        // Staged CellOS profile-manifest "lattice" matches spec "lattice-runtime"
        // (cell ProfileMatchesSpec). OCI dogfood must not invent a different default;
        // busybox OCI bundles still rely on the CellOS worker cell-agent.
        let req = ProjectionRunRequest {
            execution_mode: EXECUTION_MODE_OCI.to_string(),
            oci_bundle_path: "/tmp/cell-oci-bundles/cell_mac_live_bind".into(),
            ..ProjectionRunRequest::default()
        };
        assert_eq!(req.profile, "lattice-runtime");
        assert!(req
            .advertise_services
            .iter()
            .any(|s| s == LATTICE_RUNTIME_V1));
        assert!(req
            .advertise_services
            .iter()
            .any(|s| s == CELL_MIRROR_V1));
        assert!(is_oci_execution_mode(&req.execution_mode));
    }
}
