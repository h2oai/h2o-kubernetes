use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{Api, api::ListParams, Client, CustomResource, Error};
use kube::api::PostParams;
use serde::{Deserialize, Serialize};

use crate::templates;

pub const H2O_CRD_SINGLE_NAME: &str = "h2o";

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize)]
#[kube(group = "h2o.ai", version = "v1", kind = "H2O")]
#[kube(shortname = "h2o", namespaced)]
pub struct H2OSpec {
    pub nodes: u32,
    pub resources: Resources,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resources {
    pub cpu: u32,
    pub memory: String,
    #[serde(rename = "memoryPercentage")]
    pub memory_percentage: Option<u8>,
}

pub async fn is_deployed(client: Client) -> bool {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let items: Vec<CustomResourceDefinition> = api.list(&ListParams::default()).await
        .unwrap().items;

    return items.iter().find(|crd| {
        return crd.spec.names.kind.eq("H2O");
    }).is_some();
}

pub async fn deploy(client: Client) -> Result<(), Error> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let h2o_crd: CustomResourceDefinition = templates::h2o_crd();
    api.create(&PostParams::default(), &h2o_crd).await?;
    return Result::Ok(());
}