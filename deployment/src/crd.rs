extern crate log;

use futures::{StreamExt, TryStreamExt};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{Api, api::ListParams, Client, CustomResource, Error};
use kube::api::{DeleteParams, PostParams, WatchEvent};
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize)]
#[kube(group = "h2o.ai", version = "v1", kind = "H2O", namespaced)]
#[kube(shortname = "h2o", namespaced)]
pub struct H2OSpec {
    pub nodes: u32,
    pub resources: Resources,
}

impl H2OSpec{
    pub fn new(nodes: u32, resources: Resources) -> Self {
        H2OSpec { nodes, resources }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resources {
    pub cpu: u32,
    pub memory: String,
    #[serde(rename = "memoryPercentage", skip_serializing_if = "Option::is_none")]
    pub memory_percentage: Option<u8>,
}

impl Resources{
    pub fn new(cpu: u32, memory: String, memory_percentage: Option<u8>) -> Self {
        Resources { cpu, memory, memory_percentage }
    }
}

const H2O_RESOURCE_TEMPLATE: &str = r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: h2os.h2o.ai
spec:
  group: h2o.ai
  names:
    kind: H2O
    plural: h2os
    singular: h2o
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                nodes:
                  type: integer
                resources:
                  type: object
                  properties:
                    cpu:
                      type: integer
                      minimum: 1
                    memory:
                      type: string
                      pattern: "^([+-]?[0-9.]+)([eEinumkKMGTP]*[-+]?[0-9]*)$"
                    memoryPercentage:
                      type: integer
                      minimum: 1
                      maximum: 100
                  required: ["cpu", "memory"]
              required: ["nodes", "resources"]
"#;

const RESOURCE_NAME: &str = "h2os.h2o.ai";

pub async fn create(client: Client) -> Result<(), Error> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let h2o_crd: CustomResourceDefinition = serde_yaml::from_str(H2O_RESOURCE_TEMPLATE).unwrap();
    api.create(&PostParams::default(), &h2o_crd).await?;
    return Result::Ok(());
}

pub async fn delete(client: Client) -> Result<(), Error> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    api.delete("h2os.h2o.ai", &DeleteParams::default()).await?
        .map_left(|o| println!("Deleting CRD: {:?}", o.status))
        .map_right(|s| println!("Deleted CRD: {:?}", s));

    return Result::Ok(());
}

pub async fn exists(client: Client) -> bool {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    return api.get(RESOURCE_NAME).await.is_ok()
}

pub async fn wait_ready(client: Client, timeout: Duration) -> Result<(), Error> {
    if exists(client.clone()).await {
        return Ok(());
    }

    let api: Api<CustomResourceDefinition> = Api::all(client);
    let lp = ListParams::default()
        .fields(&format!("metadata.name={}", RESOURCE_NAME))
        .timeout(timeout.as_secs() as u32);
    let mut stream = api.watch(&lp, "0").await?.boxed();

    while let Some(status) = stream.try_next().await? {
        if let WatchEvent::Modified(s) = status {
            if let Some(s) = s.status {
                if let Some(conds) = s.conditions {
                    if let Some(pcond) = conds.iter().find(|c| c.type_ == "NamesAccepted") {
                        if pcond.status == "True" {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    // TODO: Return proper error (use anyhow ?)
    return Result::Err(Error::DynamicResource("".to_string()));
}