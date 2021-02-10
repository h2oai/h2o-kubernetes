extern crate futures;
extern crate kube;
extern crate log;
extern crate thiserror;

use kube::Client;
use kube::Error as KubeError;
use kube_runtime::watcher::Error as WatcherError;
use serde_json::Error as JsonError;
use serde_yaml::Error as YamlError;
use thiserror::Error as ThisError;

use crate::crd::H2OSpec;

pub mod crd;
pub mod finalizer;
pub mod ingress;
pub mod headless_service;
pub mod service;
pub mod statefulset;
pub mod client;
pub mod pod;

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


/// Creates all the resources necessary to start an H2O cluster according to specification.
/// Only the resources necessary for the H2O cluster to be up and running are created (exhaustive list):
/// 1. Pods, each pod with one H2O instance (one H2O JVM). With resources limits and requests set equally
/// according to the `H2OSpec` given.
/// 2. A headless service to make the clustering possible. Address of the service is provided to the underlying pods
/// via an environment variable.
///
/// The resources are invoked asynchronously and possibly in parallel. There is no guarantee the underlying
/// resources are created and the H2O cluster itself is clustered, ready and running when this function returns.
///
/// All resources share the same `name`.
///
/// # Arguments
/// - `client` - A Kubernetes client from the `kube` crate to create the resources with.
/// - `specification` - An instance of `H2OSpec` prescribing the size, resources and settings of an H2O cluster
/// - `namespace` - Namespace to deploy the H2O cluster resources to. It is the caller's responsibility to make sure
/// the client has permissions to deploy all the resources listed above into this namespace.
/// - `name` - Name of the H2O deployment.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use deployment::crd::{Resources, H2OSpec};
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// let name: &str = "test-cluster";
/// let resources: Resources = Resources::new(1, "512Mi".to_string(), Some(90));
/// let specification: H2OSpec = H2OSpec::new(
///     2,
///     Option::Some("latest".to_string()),
///     resources,
///     Option::None,
///  );
///
/// deployment::create_h2o_cluster(client, &specification, &namespace, name);
/// }
/// ```
pub async fn create_h2o_cluster(
    client: Client,
    specification: &H2OSpec,
    namespace: &str,
    name: &str,
) -> Result<(), Error> {
    let service_future = headless_service::create(client.clone(), namespace, name);
    let statefulset_future = statefulset::create(client.clone(), specification, namespace, name);
    tokio::try_join!(service_future, statefulset_future)?;
    return Ok(());
}

/// Deletes basic resources tied to an `H2O` deployment of given `name` from the Kubernetes cluster.
/// By all resources, it is meant:
/// 1. Pods with H2O nodes,
/// 2. Headless service for clustering.
///
/// No other resources are deleted.
///
/// The deletion is invoked asynchronously and potentially in parallel. Therefore, there is no guarantee
/// the resources are actually deleted at the time this function returns. The deletion itself is taken care of
/// by the respective controllers.
///
/// # Arguments
/// - `client` - A Kubernetes client from the `kube` crate to delete the resources with.
/// - `namespace` - Namespace to which the H2O cluster with given `name` has been deployed to.
/// - `name` - Name of the H2O cluster to delete.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// let name: &str = "test-cluster";
///
/// deployment::delete_h2o_cluster(client.clone(), &namespace, name).await.unwrap();
/// }
/// ```
pub async fn delete_h2o_cluster(
    client: Client,
    namespace: &str,
    name: &str,
) -> Result<(), Error> {
    let service_future = headless_service::delete(client.clone(), namespace, name);
    let statefulset_future = statefulset::delete(client.clone(), namespace, name);
    tokio::try_join!(service_future, statefulset_future)?;
    return Ok(());
}

#[cfg(test)]
mod tests {
    extern crate tests_common;

    use std::path::PathBuf;

    use k8s_openapi::api::apps::v1::StatefulSet;
    use k8s_openapi::api::core::v1::Service;
    use kube::Api;
    use kube::api::ListParams;

    use crate::crd::{H2OSpec, Resources};

    use super::kube::Client;

    use self::tests_common::kubeconfig_location_panic;

    #[tokio::test]
    async fn test_from_kubeconfig() {
        let kubeconfig_location: PathBuf = kubeconfig_location_panic();
        super::client::from_kubeconfig(kubeconfig_location.as_path()).await
            .unwrap();
    }

    #[tokio::test]
    async fn test_deploy_h2o_cluster() {
        let (client, namespace): (Client, String) = super::client::try_default().await.unwrap();
        let name: &str = "test-deploy-h2o-cluster";
        let resources: Resources = Resources::new(1, "256Mi".to_string(), Some(90));
        let specification: H2OSpec = H2OSpec::new(
            2,
            Option::Some("latest".to_string()),
            resources,
            Option::None,
        );

        super::create_h2o_cluster(client.clone(), &specification, &namespace, name)
            .await
            .unwrap();

        let ss_api: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
        assert_eq!(
            1,
            ss_api
                .list(&ListParams::default().labels(&format!("app={}", &name)))
                .await
                .unwrap()
                .items
                .len()
        );
        let service_api: Api<Service> = Api::namespaced(client.clone(), &namespace);
        assert_eq!(
            1,
            service_api
                .list(&ListParams::default().labels(&format!("app={}", &name)))
                .await
                .unwrap()
                .items
                .len()
        );

        super::delete_h2o_cluster(client.clone(), &namespace, name)
            .await
            .unwrap();
    }
}