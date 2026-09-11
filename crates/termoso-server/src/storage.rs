//! S3-compatible object storage for client-encrypted session logs.

use std::time::Duration;

use anyhow::Context;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;

use crate::config::S3Config;

#[derive(Clone)]
pub struct Storage {
    client: Client,
    /// Client used only to *sign* URLs handed to end users (public endpoint).
    presign_client: Client,
    bucket: String,
    presign_ttl: Duration,
}

pub struct Presigned {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub expires_in: u64,
}

impl Storage {
    pub async fn new(cfg: &S3Config) -> anyhow::Result<Self> {
        let build = |endpoint: Option<&str>| {
            let creds = Credentials::new(&cfg.access_key, &cfg.secret_key, None, None, "termoso");
            let mut b = aws_sdk_s3::Config::builder()
                .behavior_version(BehaviorVersion::latest())
                .credentials_provider(creds)
                .region(Region::new(
                    cfg.region.clone().unwrap_or_else(|| "us-east-1".into()),
                ))
                .force_path_style(cfg.force_path_style);
            if let Some(ep) = endpoint {
                b = b.endpoint_url(ep);
            }
            Client::from_conf(b.build())
        };
        let client = build(cfg.endpoint.as_deref());
        let presign_client = build(cfg.public_endpoint.as_deref().or(cfg.endpoint.as_deref()));
        Ok(Self {
            client,
            presign_client,
            bucket: cfg.bucket.clone(),
            presign_ttl: Duration::from_secs(cfg.presign_secs),
        })
    }

    pub async fn ensure_bucket(&self) -> anyhow::Result<()> {
        match self.client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => Ok(()),
            Err(_) => {
                self.client
                    .create_bucket()
                    .bucket(&self.bucket)
                    .send()
                    .await
                    .with_context(|| format!("creating bucket {}", self.bucket))?;
                Ok(())
            }
        }
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await?;
        Ok(())
    }

    fn presign_cfg(&self) -> anyhow::Result<PresigningConfig> {
        Ok(PresigningConfig::expires_in(self.presign_ttl)?)
    }

    pub async fn presign_put(&self, key: &str, max_len: i64) -> anyhow::Result<Presigned> {
        let req = self
            .presign_client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_length(max_len)
            .presigned(self.presign_cfg()?)
            .await?;
        Ok(Presigned {
            url: req.uri().to_string(),
            headers: req
                .headers()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            expires_in: self.presign_ttl.as_secs(),
        })
    }

    pub async fn presign_get(&self, key: &str) -> anyhow::Result<Presigned> {
        let req = self
            .presign_client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(self.presign_cfg()?)
            .await?;
        Ok(Presigned {
            url: req.uri().to_string(),
            headers: Vec::new(),
            expires_in: self.presign_ttl.as_secs(),
        })
    }

    /// Actual size of an uploaded object, `None` if it does not exist.
    pub async fn object_size(&self, key: &str) -> anyhow::Result<Option<i64>> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(head) => Ok(head.content_length()),
            Err(e) => {
                if e.as_service_error()
                    .map(|s| s.is_not_found())
                    .unwrap_or(false)
                {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }
}
