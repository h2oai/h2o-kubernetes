use kube::Error as KubeError;
use kube_runtime::watcher::Error as WatcherError;
use serde_json::Error as JsonError;
use serde_yaml::Error as YamlError;
use thiserror::Error as ThisError;
use reqwest::Error as ReqwestError;

pub mod crd;
pub mod finalizer;
pub mod service;
pub mod client;
pub mod pod;
pub mod configmap;

/// Error during handling Kubernetes cluster-related requests.
#[derive(ThisError, Debug)]
pub enum Error {
    /// Error originating from the Kubernetes API and/or the `kube` crate
    #[error("Kubernetes reported error: {0}")]
    KubeError(KubeError),
    /// Error in user-provided data/configuration
    #[error("Kubernetes reported error: {0}")]
    UserError(String),
    /// Requested operation timed out
    #[error("Operation timed out. Reason: {0}")]
    Timeout(String),
    #[error("Failed to serialize template. Reason: {0}")]
    TemplateSerializationError(String),
    #[error("Resource watch failed. Reason: {0}")]
    WatcherError(WatcherError),
    #[error("Error during H2O subresources deployment: {0}")]
    DeploymentError(String),
    #[error("Hyper error: {0}")]
    ReqwestError(ReqwestError),
}

impl From<KubeError> for Error {
    fn from(kube_error: KubeError) -> Self {
        Error::KubeError(kube_error)
    }
}

impl From<YamlError> for Error {
    fn from(yaml_error: YamlError) -> Self {
        Error::TemplateSerializationError(yaml_error.to_string())
    }
}

impl From<JsonError> for Error {
    fn from(json_error: JsonError) -> Self {
        Error::TemplateSerializationError(json_error.to_string())
    }
}

impl From<WatcherError> for Error {
    fn from(watcher_error: WatcherError) -> Self {
        Error::WatcherError(watcher_error)
    }
}

impl From<ReqwestError> for Error {
    fn from(reqwest_error: ReqwestError) -> Self {
        Error::ReqwestError(reqwest_error)
    }
}