use kube::{Api, Client};
use kube::api::{PatchParams, PatchStrategy};
use serde_json::json;

use crate::crd::H2O;
use crate::Error;

pub const FINALIZER_NAME: &str = "h2o3.h2o.ai";

/// Adds a finalizer into metadata of an H2O resource of given `name`.
/// The resource modification is an asynchronous operation - at the time this method returns,
/// it is not guaranteed the resource will contain the finalizer.
///
/// # Arguments
/// `client` - Client to Kubernetes API with sufficient permissions to modify the resource
/// `namespace` - Namespace the `H2O` resource is deployed to.
/// `name` - Name of the resource to modify.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// deployment::finalizer::add_finalizer(client, &namespace, "any-name").await.unwrap();
/// }
/// ```
pub async fn add_finalizer(client: Client, namespace: &str, name: &str) -> Result<H2O, Error> {
    let h2o_api: Api<H2O> = Api::namespaced(client, namespace);
    let finalizer = json!({
        "metadata": {
            "finalizers": ["h2o3.h2o.ai"]
        }
    });

    let patch_params: PatchParams = PatchParams {
        dry_run: false,
        patch_strategy: PatchStrategy::Merge,
        force: false,
        field_manager: None,
    };
    return h2o_api
        .patch(name, &patch_params, serde_json::to_vec(&finalizer)
            .map_err(Error::from_serde_json_error)?)
        .await
        .map_err(Error::from_kube_error);
}

/// Removes a finalizer from metadata of an H2O resource of given `name`.
/// This is an asynchronous operation - at the time this method returns, there is no guarantee
/// the finalizer will be removed from the resource.
///
/// # Arguments
/// `client` - Client to Kubernetes API with sufficient permissions to modify the resource
/// `namespace` - Namespace the `H2O` resource is deployed to.
/// `name` - Name of the resource to modify.
pub async fn remove_finalizer(client: Client, name: &str, namespace: &str) -> Result<H2O, Error> {
    let h2o_api: Api<H2O> = Api::namespaced(client, namespace);
    let finalizer = json!({
        "metadata": {
            "finalizers": null
        }
    });

    let patch_params: PatchParams = PatchParams {
        dry_run: false,
        patch_strategy: PatchStrategy::Merge,
        force: false,
        field_manager: None,
    };

    return h2o_api
        .patch(name, &patch_params, serde_json::to_vec(&finalizer)
            .map_err(Error::from_serde_json_error)?)
        .await
        .map_err(Error::from_kube_error);
}
