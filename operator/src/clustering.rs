use std::borrow::{BorrowMut};
use std::net::{IpAddr, SocketAddr};

use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};
use kube::api::PatchParams;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use log::{info, debug};
use reqwest::{Client as ReqwestClient, Response};

use deployment::Error;
use std::str::FromStr;

pub async fn cluster_pods(client: Client, namespace: &str, pod_label: &str, expected_pod_count: usize) {
    let pod_has_ip_check: fn(&Pod) -> bool = |pod| {
        if let Some(status) = pod.status.as_ref() {
            return status.pod_ip.is_some();
        }
        false
    };

    let created_pods: Vec<Pod> = deployment::pod::wait_pod_status(client.clone(), pod_label, namespace,
                                                                  expected_pod_count as usize,
                                                                  pod_has_ip_check,
    ).await;

    let pod_ips: Vec<IpAddr> = created_pods.iter()
        .map(|pod| {
            let pod_ip = pod.status.as_ref()
                .expect("Pod expected to have a status entry.")
                .pod_ip.as_ref()
                .expect("Pod expected to have ClusterIP assigned.");
            IpAddr::from_str(pod_ip).unwrap()
        })
        .collect();


    let reqwest: ReqwestClient = ReqwestClient::new();
    wait_clustering_api_online(&pod_ips, &reqwest, pod_label).await;
    send_flatfile(&pod_ips, &reqwest).await;
    let leader_node_timeout = tokio::time::timeout(Duration::from_secs(180), wait_h2o_clustered(&reqwest, &pod_ips)).await;
    let leader_node_socket_addr: SocketAddr = leader_node_timeout.unwrap().unwrap(); // TODO: Remove unwrap

    let mut leader_node_pod: Pod = created_pods.into_iter()
        .find(|pod| {
            let pod_ip = pod.status.as_ref().unwrap()
                .pod_ip.as_ref().unwrap();
            return pod_ip == &leader_node_socket_addr.ip().to_string();
        }).unwrap();

    let leader_node_label: String = format!("{}-leader", pod_label);

    leader_node_pod.borrow_mut()
        .metadata
        .labels
        .as_mut()
        .unwrap()
        .insert("h2o_leader_node_pod".to_owned(), leader_node_label.clone());

    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    api.patch_status(&leader_node_pod.metadata.name.as_ref().unwrap(), &PatchParams::default(), serde_json::to_vec(&leader_node_pod).unwrap()).await.unwrap();

    deployment::service::create(client, namespace, pod_label, &format!("{}-leader", pod_label)).await.unwrap(); // TODO: Remove unwrap
}

async fn wait_clustering_api_online(pod_ips: &[IpAddr], reqwest: &ReqwestClient, pod_label: &str) {
    let mut all_pods_apis_ready: bool = false;

    while !all_pods_apis_ready {
        all_pods_apis_ready = futures::stream::iter(0..pod_ips.len())
            .map(|pod_ip_idx| {
                clustering_api_available(reqwest, &pod_ips[pod_ip_idx])
            })
            .buffer_unordered(pod_ips.len())
            .fold(true, |r1, r2| {
                futures::future::ready(r1 && r2)
            }).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn clustering_api_available(reqwest: &ReqwestClient, pod_ip: &IpAddr) -> bool {
    let response = reqwest.get(&format!("http://{}:{}/cluster/status", pod_ip.to_string(), deployment::pod::H2O_CLUSTERING_PORT))
        .send()
        .await;

    return match response {
        Ok(response) => {
            response.status() == 204
        }
        Err(_) => {
            false
        }
    };
}

async fn send_flatfile(pod_ips: &[IpAddr], reqwest: &ReqwestClient) -> bool { // TODO: Parse to IpAddr
    let flatfile: String = create_flatfile(pod_ips);
    // Send all flat files to all H2O nodes concurrently.
    futures::stream::iter(0..pod_ips.len()).map(|pod_index| {
        let pod_ip = &pod_ips[pod_index];
        info!("Sending flatfile to: {}", format!("http://{}:8080/clustering/flatfile", pod_ip.to_string()));
        reqwest.post(&format!("http://{}:{}/clustering/flatfile", pod_ip.to_string(), deployment::pod::H2O_CLUSTERING_PORT))
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(flatfile.clone())
            .send()
    }).buffer_unordered(pod_ips.len())
        .map(|result| {
            result.unwrap().status() == 200
        })
        .fold(true, |a, b| {
            futures::future::ready(a && b)
        })
        .await
}

fn create_flatfile(pod_ipds: &[IpAddr]) -> String {
    pod_ipds.iter()
        .map(|pod_ip| {
            let pod_socket_addr = SocketAddr::new(pod_ip.clone(), deployment::pod::H2O_DEFAULT_PORT);
            pod_socket_addr.to_string()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[derive(Serialize, Deserialize)]
struct H2OClusterStatus {
    leader_node: String,
    healthy_nodes: Vec<String>,
    unhealthy_nodes: Vec<String>,
}

async fn wait_h2o_clustered(reqwest: &ReqwestClient, pod_ips: &[IpAddr]) -> Result<SocketAddr, Error> {
    let h2o_pod_ip = pod_ips.get(0).expect("Expected H2O cluster to have at least one node."); // TODO: Rule out this possibility of empty cluster - add a proper reaction

    let cluster_status: H2OClusterStatus;
    'clustering: loop {
        let cluster_status_response = reqwest.get(&format!("http://{}:8080/cluster/status", h2o_pod_ip))
            .send().await;
        match cluster_status_response {
            Ok(status) => {
                if status.status() != 200 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue 'clustering;
                } else {
                    cluster_status = status.json().await?;
                    break 'clustering;
                }
            }
            Err(err) => {
                continue 'clustering;
            }
        }
    }

    // TODO: Check status of all nodes

    return Ok(cluster_status.leader_node.parse().unwrap()); //TODO: Remove unwrap
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_cluster_pods() {}
}