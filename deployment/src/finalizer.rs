use kube::{Client, Api, Error};
use kube::api::{PatchParams, PatchStrategy};
use serde_json::json;
use crate::crd::H2O;

pub const FINALIZER_NAME: &str = "h2o3.h2o.ai";

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
    return h2o_api.patch(name, &patch_params, serde_json::to_vec(&finalizer).unwrap()).await;
}

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
    return h2o_api.patch(name, &patch_params, serde_json::to_vec(&finalizer).unwrap()).await;
}