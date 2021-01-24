use std::borrow::{Borrow, BorrowMut};
use std::net::{IpAddr, SocketAddr};

use futures::StreamExt;
use hyper::{Body, Client as HyperClient, Method, Request, Response, StatusCode};
use hyper::client::{HttpConnector, ResponseFuture};
use hyper::header::CONTENT_TYPE;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};
use kube::api::PatchParams;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use deployment::Error;

pub async fn cluster_pods(client: Client, namespace: &str, pod_label: &str, expected_pod_count: usize) {
    let pod_has_ip_check: fn(&Pod) -> bool = |pod| {
        if let Some(status) = pod.status.as_ref() {
            return status.pod_ip.is_some();
        }
        false
    };

    let created_pods: Vec<Pod> = deployment::pod::wait_pods_created(client.clone(), pod_label, namespace,
                                                          expected_pod_count as usize,
                                                          pod_has_ip_check,
    ).await;

    let pod_ips: Vec<String> = created_pods.iter()
        .map(|pod| {
            pod.status.as_ref()
                .expect("Pod expected to have a status entry.")
                .pod_ip.as_ref()
                .expect("Pod expected to have ClusterIP assigned.")
                .clone()
        })
        .collect();

    let http_client: HyperClient<HttpConnector> = HyperClient::new();
    send_flatfile(pod_ips.as_slice(), &http_client).await;
    let leader_node_timeout = tokio::time::timeout(Duration::from_secs(180), wait_h2o_clustered(&http_client, &pod_ips)).await;
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

async fn send_flatfile(pod_ips: &[String], http_client: &HyperClient<HttpConnector>) -> bool { // TODO: Parse to IpAddr

    // Send all flat files to all H2O nodes concurrently.
    futures::stream::iter(0..pod_ips.len()).map(|pod_index| {
        let pod_ip = &pod_ips[pod_index];
        let request = Request::builder()
            .method(Method::POST)
            .uri(format!("http://{}:8080/clustering/flatfile", pod_ip))
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::from("")).unwrap(); // TODO: remove unwrap
        http_client.request(request)
    }).buffer_unordered(pod_ips.len())
        .map(|result| {
            result.unwrap().status() == 200
        })
        .fold(true, |a, b| {
            futures::future::ready(a && b)
        })
        .await
}

#[derive(Serialize, Deserialize)]
struct H2OClusterStatus {
    leader_node: String,
    healthy_nodes: Vec<String>,
    unhealthy_nodes: Vec<String>,
}

async fn wait_h2o_clustered(http_client: &HyperClient<HttpConnector>, pod_ips: &[String]) -> Result<SocketAddr, Error> {
    let h2o_pod_ip = pod_ips.get(0).expect("Expected H2O cluster to have at least one node."); // TODO: Rule out this possibility of empty cluster - add a proper reaction

    let cluster_status: H2OClusterStatus;
    'clustering: loop {
        let cluster_status_request = Request::builder()
            .uri(format!("http://{}:8080/cluster/status", h2o_pod_ip))
            .body(Body::empty()).unwrap(); // TODO: Remove unwrap
        let cluster_status_response = http_client.request(cluster_status_request).await;
        match cluster_status_response {
            Ok(status) => {
                if (status.status() != 200) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue 'clustering;
                } else {
                    cluster_status = serde_json::from_slice(&hyper::body::to_bytes(status.into_body()).await.unwrap()).unwrap();
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