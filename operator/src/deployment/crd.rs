extern crate log;

use kube::{Api, Client, CustomResource};
use kube::api::{PatchParams, ListParams};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::{Error};
use futures::StreamExt;
use kube_runtime::watcher::Event;
use log::info;
use crate::deployment::finalizer::FINALIZER_NAME;

/// Specification of an H2O cluster in a Kubernetes cluster.
/// Determines attributes like cluster size, resources (cpu, memory) and pod configuration.
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(group = "h2o.ai", version = "v1beta", kind = "H2O", status = "H2OStatus", derive = "PartialEq", namespaced)]
#[kube(shortname = "h2o", namespaced)]
pub struct H2OSpec {
    pub nodes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub resources: Resources,
    #[serde(rename = "customImage", skip_serializing_if = "Option::is_none")]
    pub custom_image: Option<CustomImage>,
}

impl H2OSpec {
    /// Constructor pattern for `H2OSpec`
    ///
    /// # Arguments
    /// `nodes` - Number of H2O node to spawn. Directly translates to a number of pods, as there is one
    /// H2O per pod.
    /// `version` - H2O Version - used as a tag for the official Docker image, unless overridden by
    /// a custom image. The tag must be present in [H2O Docker Hub repository](https://hub.docker.com/r/h2oai/h2o-open-source-k8s)
    /// `resources` - Per-pod resources to be allocated for H2O pods.
    /// `custom_image` - Custom image with H2O inside to be used. User takes full responsibility for image correctness.
    pub fn new(
        nodes: u32,
        version: Option<String>,
        resources: Resources,
        custom_image: Option<CustomImage>,
    ) -> Self {
        H2OSpec {
            nodes,
            version,
            resources,
            custom_image,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct H2OStatus {
    phase: Option<String>,
    conditions: Option<Vec<Condition>>,
}

impl H2OStatus {
    pub fn new(phase: Option<String>, conditions: Option<Vec<Condition>>) -> Self {
        H2OStatus { phase, conditions }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Condition {
    #[serde(rename = "type")]
    cond_type: String,
    status: String,
}

impl Condition {
    pub fn new(cond_type: String, status: String) -> Self {
        Condition { cond_type, status }
    }
}


/// Resources allocated by each H2O pod
/// Limits and requests are always set to the same value in order for H2O operations
/// tobe reproducible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Resources {
    /// Number of virtual CPUs allocated to each H2O pod
    pub cpu: u32,
    /// A Kubernetes-compliant memory string matching the following pattern: `^([+-]?[0-9.]+)([eEinumkKMGTP]*[-+]?[0-9]*)$`.
    pub memory: String,
    /// Percentage of memory allocated by the H2O JVM inside the docker container running
    /// inside the pod. If not defined, defaults will be used. Unless external XGBoost is always spawned,
    /// there will always be some space required for XGBoost.
    #[serde(rename = "memoryPercentage", skip_serializing_if = "Option::is_none")]
    pub memory_percentage: Option<u8>,
}

impl Resources {
    /// Constructor for `Resources`
    ///
    /// # Arguments
    /// `cpu` - Number of virtual CPUs allocated to each H2O pod
    /// `memory` - A Kubernetes-compliant memory string matching the following pattern: `^([+-]?[0-9.]+)([eEinumkKMGTP]*[-+]?[0-9]*)$`
    /// `memory_percentage` - Optional percentage of memory allocated by the H2O JVM inside the docker container running inside the pod
    pub fn new(cpu: u32, memory: String, memory_percentage: Option<u8>) -> Self {
        Resources {
            cpu,
            memory,
            memory_percentage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct CustomImage {
    /// Full image definition, including repository prefix, image name and tag.
    pub image: String,
    /// Docker command to be ran when the custom image is started.
    pub command: Option<String>,
}

impl CustomImage {
    /// Constructor for `CustomImage`
    ///
    /// # Arguments
    /// `image` - Full image definition, including repository prefix, image name and tag.
    /// `command` - Optional Docker command to be ran when the custom image is started.
    pub fn new(image: String, command: Option<String>) -> Self {
        CustomImage { image, command }
    }
}

/// Scans `H2O` resources and returns `true` if there is a deletion timestamp present in the resource's
/// metadata. Returns `false` if there is no deletion timestamp.
///
/// If there is a deletion timestamp, a `delete` command has been issued on that `H2O` resources.
///
/// # Arguments
///
/// `h2o` - The `H2O` resource instance, representing the current state of the resource in Kubernetes cluster.
pub fn has_deletion_stamp(h2o: &H2O) -> bool {
    return h2o.metadata.deletion_timestamp.is_some();
}

pub fn is_ready(h2o: &H2O) -> bool {
    if h2o.status.is_none(){
        return false;
    }
    let status: &H2OStatus = h2o.status.as_ref().unwrap();
    return match status.conditions.as_ref() {
        None => { false }
        Some(conditions) => {
            conditions.iter()
                .filter(|condition| {
                    info!("{:?}", condition);
                    condition.cond_type.eq("Ready")
                    && condition.status.eq("true")
                }).count() > 0
        }
    }
}

/// Scans `H2O` resource and returns `true` if there is a finalizer intended to be handled
/// by this operator in the resource's metadata. If there is no such finalizer, returns `false`.
///
/// If no finalizer is present, this typically indicates the resources has just been created and not handled
/// by this operator yet, as during the first reconciliation, the finalizer is **always** added.
///
/// # Arguments
///
/// `h2o` - The `H2O` resource instance, representing the current state of the resource in Kubernetes cluster.
pub fn has_h2o3_finalizer(h2o: &H2O) -> bool {
    return match h2o.metadata.finalizers.as_ref() {
        Some(finalizers) => {
            finalizers.contains(&String::from(FINALIZER_NAME))
        }
        None => false,
    };
}

pub async fn set_ready_condition(client: Client, name: &str, namespace: &str, ready: bool) -> Result<H2O, Error> {
    let api: Api<H2O> = Api::namespaced(client.clone(), namespace);
    let mut h2o: H2O = api.get(name).await.unwrap();
    let condition: Condition = Condition::new("Ready".to_owned(), ready.to_string());
    let status: H2OStatus = H2OStatus::new(Some("Running".to_owned()), Some(vec!(condition)));
    h2o.status = Option::Some(status);

    let result: Result<H2O, Error> = api.patch_status(name, &PatchParams::default(), serde_json::to_vec(&h2o)
        .unwrap()).await
        .map_err(Error::from);

    return result;
}

pub async fn wait_ready_h2o(client: Client, name: &str, namespace: &str) {
    let api: Api<H2O> = Api::namespaced(client.clone(), namespace);

    let list_params: ListParams = ListParams::default()
        .fields(&format!("metadata.name={}", name));
    let mut pod_events = kube_runtime::watcher(api, list_params).boxed();

    while let Some(result) = pod_events.next().await {
        match result {
            Ok(event) => {
                match event {
                    Event::Applied(h2o) =>{
                        if h2o.status.is_none() {
                            continue;
                        }
                        if h2o.status.unwrap().conditions.is_some(){
                            break;
                        }
                    },
                    Event::Restarted(h2os) => {
                        for h2o in h2os {
                            if h2o.status.is_none() {
                                continue;
                            }
                            if h2o.status.unwrap().conditions.is_some(){ // TODO check all options
                                break;
                            }
                        }
                    },
                    Event::Deleted(_)=>{
                    }
                }
            },
            Err(_) => {}
        }
    }
}

pub async fn wait_deleted_h2o(client: Client, name: &str, namespace: &str) {
    let api: Api<H2O> = Api::namespaced(client.clone(), namespace);

    let list_params: ListParams = ListParams::default()
        .fields(&format!("metadata.name={}", name));
    let mut pod_events = kube_runtime::watcher(api, list_params).boxed();

    while let Some(result) = pod_events.next().await {
        match result {
            Ok(event) => {
                match event {
                    Event::Deleted(_)=>{
                        break;
                    },
                    _ => {
                    }
                }
            },
            Err(_) => {}
        }
    }
}