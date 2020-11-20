extern crate deployment;
extern crate tests_common;

use std::collections::HashMap;
use std::process::{Child, Command};

use futures::{StreamExt};
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{Api, Client, Error};
use kube::api::{DeleteParams, ListParams, Meta, PostParams, WatchEvent};
use log::info;

use deployment::crd::{H2O, H2OSpec, Resources, CRDReadiness};
use std::time::Duration;
use std::path::PathBuf;

#[tokio::test]
async fn test_operator_deploy_undeploy() {
    let kubeconfig_location : PathBuf = tests_common::kubeconfig_location_panic();
    let mut h2o_operator_process: Child = start_h2o_operator(kubeconfig_location.to_str().unwrap());
    let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
    deployment::crd::wait_crd_status(client.clone(), Duration::from_secs(180), CRDReadiness::Ready).await.expect("CRD not available within timeout.");

    let h2o_api: Api<H2O> = Api::namespaced(client.clone(), &namespace);
    let h2o_name = "test-operator-deploy-undeploy";
    let node_count: usize = 3;

    // Create H2O in Kubernetes cluster
    let resources: Resources = Resources::new(1, "256Mi".to_string(), Option::None);
    let h2o_spec: H2OSpec = H2OSpec::new(node_count as u32, Option::Some("latest".to_string()), resources, Option::None);
    let h2o: H2O = H2O::new(h2o_name, h2o_spec);
    h2o_api.create(&PostParams::default(), &h2o).await.unwrap();

    // Wait for H2O pods to be created
    let pods: Vec<Pod> = wait_pods_created(client.clone(), h2o_name, &namespace, node_count).await;
    assert_eq!(node_count, pods.len());
    pods.iter().for_each(|pod| {
        info!("{:?}", pod);
    });

    // Check the service has been created as well
    let service_api: Api<Service> = Api::namespaced(client.clone(), &namespace);
    let service: Service = service_api.get(h2o_name).await.unwrap();
    assert!(service.spec.unwrap().cluster_ip.unwrap().eq("None")); // The service created must be a headless service - thus no cluster ip

    h2o_api.delete(h2o_name, &DeleteParams::default()).await.unwrap();

    assert!(wait_pods_deleted(client.clone(), h2o_name, &namespace).await.is_ok());

    deployment::crd::delete(client.clone()).await.unwrap();
    deployment::crd::wait_crd_status(client.clone(), Duration::from_secs(180), CRDReadiness::Unready).await.unwrap();

    h2o_operator_process.kill().unwrap();
}

fn start_h2o_operator(kubeconfig_location: &str) -> Child {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("h2o-operator"));
    cmd.env("KUBECONFIG", kubeconfig_location);
    return cmd.spawn().unwrap();
}

async fn wait_pods_created(client: Client, name: &str, namespace: &str, expected_count: usize) -> Vec<Pod> {
    let api: Api<Pod> = Api::<Pod>::namespaced(client.clone(), namespace);
    let list_params: ListParams = ListParams::default()
        .labels(&format!("app={}", name))
        .timeout(180);

    let mut pod_watcher = api.watch(&list_params, "0").await.unwrap().boxed();
    let mut discovered_pods: HashMap<String, Pod> = HashMap::with_capacity(expected_count);

    while let Some(result) = pod_watcher.next().await {
        match result {
            Ok(event) => {
                match event {
                    WatchEvent::Added(pod) => {
                        discovered_pods.insert(pod.name().clone(), pod);
                        if discovered_pods.len() == expected_count {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    };

    // Pods do not support `Eq` for HashSets, return as plain vector
    let pods = discovered_pods.values().map(|entry| {
        entry.clone()
    }).collect::<Vec<Pod>>();

    return pods;
}

async fn wait_pods_deleted(client: Client, name: &str, namespace: &str) -> kube::Result<(), Error> {
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list_params: ListParams = ListParams {
        label_selector: Some(format!("app={}", name)),
        field_selector: None,
        timeout: Some(120),
        allow_bookmarks: false,
        limit: None,
        continue_token: None,
    };

    let mut pod_count: usize = pod_api.list(&pod_list_params).await.unwrap().items.len();
    info!("Waiting to delete {} pods.", pod_count);
    if pod_count == 0 { return Result::Ok(()); }

    let mut stream = pod_api.watch(&pod_list_params, "0").await?.boxed();
    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                match event {
                    WatchEvent::Deleted(_) | WatchEvent::Error(_) => {
                        pod_count = pod_count - 1;
                        if pod_count == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    };
    return Result::Ok(());
}
