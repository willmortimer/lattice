//! [`HttpCloudBlobClient`] — production [`CloudBlobClient`] over bearer auth.

use latticefs_core::{CloudBlobClient, ContentHash, Error as FsError, ResourceId, Result as FsResult};

use crate::client::CloudApiClient;
use crate::error::CloudError;

pub struct HttpCloudBlobClient<C: crate::client::CloudHttpClient> {
    api: CloudApiClient<C>,
    bearer: String,
}

impl<C: crate::client::CloudHttpClient> HttpCloudBlobClient<C> {
    pub fn new(api: CloudApiClient<C>, bearer: impl Into<String>) -> Self {
        Self {
            api,
            bearer: bearer.into(),
        }
    }

    pub fn api(&self) -> &CloudApiClient<C> {
        &self.api
    }
}

impl<C: crate::client::CloudHttpClient> CloudBlobClient for HttpCloudBlobClient<C> {
    fn put_blob(&self, resource_id: ResourceId, data: &[u8]) -> FsResult<ContentHash> {
        self.api
            .put_blob(&self.bearer, resource_id, data)
            .map_err(map_fs_error)
    }

    fn get_blob(&self, resource_id: ResourceId) -> FsResult<Vec<u8>> {
        self.api
            .get_blob(&self.bearer, resource_id)
            .map_err(map_fs_error)
    }
}

fn map_fs_error(err: CloudError) -> FsError {
    FsError::CloudBlob {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use latticefs_core::{roundtrip_verify_blob, ContentHash, ResourceId};

    use super::*;
    use crate::client::{
        CloudApiClient, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse,
    };

    #[derive(Default, Clone)]
    struct BlobFakeHttp {
        json: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
        bytes: Arc<Mutex<HashMap<String, CloudHttpBytesResponse>>>,
        stored: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    }

    impl BlobFakeHttp {
        fn with_blob(&self, resource_id: ResourceId, data: &[u8], hash_hex: &str) {
            let path = format!("/v1/blobs/{resource_id}");
            let put_key = format!("PUT {path}");
            let get_key = format!("GET {path}");
            self.bytes.lock().unwrap().insert(
                put_key,
                CloudHttpBytesResponse {
                    status: 201,
                    body: format!(
                        r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/{resource_id}","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                        data.len()
                    )
                    .into_bytes(),
                    content_hash: None,
                },
            );
            self.bytes.lock().unwrap().insert(
                get_key,
                CloudHttpBytesResponse {
                    status: 200,
                    body: data.to_vec(),
                    content_hash: Some(hash_hex.to_string()),
                },
            );
            self.stored
                .lock()
                .unwrap()
                .insert(resource_id.to_string(), data.to_vec());
        }
    }

    impl CloudHttpClient for BlobFakeHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&serde_json::Value>,
            _bearer: Option<&str>,
        ) -> crate::Result<CloudHttpResponse> {
            self.json
                .lock()
                .unwrap()
                .get(&format!("{method} {path}"))
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake json for {method} {path}")))
        }

        fn request_bytes(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            body: Option<&[u8]>,
            bearer: Option<&str>,
            headers: &[(&str, &str)],
        ) -> crate::Result<CloudHttpBytesResponse> {
            if bearer != Some("good-token") {
                return Ok(CloudHttpBytesResponse {
                    status: 401,
                    body: br#"{"error":"invalid session"}"#.to_vec(),
                    content_hash: None,
                });
            }
            let key = format!("{method} {path}");
            if method == "PUT" {
                if let Some(data) = body {
                    let hash_header = headers
                        .iter()
                        .find(|(name, _)| *name == "X-Lattice-Content-Hash")
                        .map(|(_, value)| *value)
                        .expect("hash header");
                    let computed_hash = ContentHash::from_bytes(data).expect("hash");
                    let computed = computed_hash
                        .as_str()
                        .strip_prefix("sha256:")
                        .unwrap();
                    assert_eq!(hash_header, computed);
                    self.stored.lock().unwrap().insert(
                        path.trim_start_matches("/v1/blobs/").to_string(),
                        data.to_vec(),
                    );
                }
            }
            self.bytes
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake bytes for {key}")))
        }
    }

    #[test]
    fn http_blob_roundtrip_with_fake_transport() {
        let http = BlobFakeHttp::default();
        let resource_id = ResourceId::new();
        let data = b"opaque-cloud-bytes";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = hash.as_str().strip_prefix("sha256:").unwrap();
        http.with_blob(resource_id, data, hash_hex);

        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let blob_client = HttpCloudBlobClient::new(client, "good-token");
        let hash = roundtrip_verify_blob(&blob_client, resource_id, data).unwrap();
        assert_eq!(hash, ContentHash::from_bytes(data).unwrap());
    }
}
