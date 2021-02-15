use deployment::crd::H2OSpec;
use kube::{Client, Api};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams};
use log::{error};
use std::net::{IpAddr, SocketAddr};
use reqwest::{Client as ReqwestClient, Response};
use serde::{Serialize, Deserialize};
use deployment::Error;
use std::str::FromStr;
use futures::StreamExt;
use std::time::Duration;

pub async fn check_h2o_cluster_integrity(client: Client, name: &str, namespace: &str, h2o_spec: &H2OSpec) -> bool {
    return cluster_healthy(client.clone(), namespace, name, h2o_spec.nodes).await;
}

async fn cluster_healthy(client: Client, namespace: &str, pod_label: &str, node_count: u32) -> bool {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list_params: ListParams = ListParams::default()
        .labels(&format!("app={}", pod_label));
    let pods_result = api.list(&pod_list_params).await;
    let reqwest: ReqwestClient = ReqwestClient::new();

    if let Err(err) = pods_result {
        error!("Cluster health check failed. Unable to list H2O pods. Error:\n{}", err);
        return false;
    }
    let pods: Vec<Pod> = pods_result.unwrap().items;
    if pods.len() != node_count as usize{
        return false;
    }

    return futures::stream::iter(0..pods.len())
        .map(|pod_idx| {
            let pod: &Pod = &pods[pod_idx];
            let pod_ip: IpAddr = IpAddr::from_str(pod.status.as_ref().unwrap().pod_ip.as_ref().unwrap()).unwrap();
            pod_status(pod_ip, &reqwest)
        })
        .buffer_unordered(pods.len())
        .map(|h2o_status_result| {
            return match h2o_status_result {
                Ok(h2o_status) => {
                    is_node_healthy(&h2o_status, node_count as usize)
                }
                Err(err) => {
                    error!("Error obtaining H2O node health status. Error:\n{}", err);
                    false
                }
            };
        })
        .fold(true, |a, b| {
            futures::future::ready(a && b)
        }).await;
}

#[derive(Deserialize, Serialize)]
pub struct H2ONodeStatus {
    leader_node: SocketAddr,
    healthy_nodes: Vec<SocketAddr>,
    unhealthy_nodes: Vec<SocketAddr>,
}

pub async fn pod_status(pod_ip: IpAddr, reqwest: &ReqwestClient) -> Result<H2ONodeStatus, Error> {
    let pod_status: H2ONodeStatus = reqwest.get(&format!("http://{}:{}/cluster/status", pod_ip, deployment::pod::H2O_CLUSTERING_PORT))
        .send()
        .await?
        .json()
        .await?;

    Ok(pod_status)
}

fn is_node_healthy(pod_status: &H2ONodeStatus, expected_size: usize) -> bool {
    pod_status.healthy_nodes.len() == expected_size && pod_status.unhealthy_nodes.is_empty()
}