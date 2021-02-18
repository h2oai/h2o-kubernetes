use kube::Client;
use log::{info, error, LevelFilter};
use simple_logger::SimpleLogger;

use deployment::Error;
use std::path::{PathBuf, Path};
use std::str::FromStr;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::Meta;
use crate::deployment::configmap;

mod controller;
mod clustering;
mod verification;
mod deployment;

/// Entrypoint to H2O Open Source Kubernetes operator executable. This operator acts upon H2O-related
/// Custom Resource Definitions (CRDs), handling their state changes, creation and deletion.
///
/// # Before controller is ran
///
/// 1. First, utility libraries (logging etc.) are initialized.
/// 2. An attempt to obtain a Kubernetes client from a Kubeconfig is made.
/// 3. H2O Custom resource definition (CRD) presence in cluster is detected. If not present
///     attempt to deploy it is made. If unsuccessful (permissions), the operator shuts down.
///
/// # Controller
///
/// The controller structure itself comes from `kube*` crates, specifically from the [kube-runtime](https://crates.io/crates/kube-runtime) crate.
/// These are Rust's Kubernetes client libraries.
/// It runs in an endless loop, dispatching incoming requests for changes regarding H2O's CRDs to
/// custom logic.
///
/// # Asynchronous execution
///
/// The whole operator uses asynchronous code, as Kubernetes itself (and thus the `kube` client) are
/// asynchronous as well. In Rust, a runtime has to bee selected by the user, as the core [async/await](https://rust-lang.github.io/async-book/01_getting_started/01_chapter.html)
/// functionality from the standard library is runtime-agnostic. [Tokio](https://tokio.rs/) is a widely-used library
/// which provides such Runtime.
///
/// There are two basic types of executors - single-threaded executor (one context switching OS-level thread)
/// and a multi-threaded executor. The multi-threaded executor is [enabled by default](https://docs.rs/tokio/0.3.3/tokio/attr.main.html)
/// and defaults to number of detected CPUs. To ensure optimal utilization of resources, the default option is kept.
///
#[tokio::main]
async fn main() -> Result<(), Error> {
    initialize_logging();
    info!("H2O Kubernetes Operator");
    let (client, namespace): (Client, String) = deployment::client::try_default().await?;
    print_startup_diagnostics(&client, &namespace).await;
    create_mandatory_resources(client.clone(), &namespace).await;
    controller::run(client.clone(), &namespace).await;
    Ok(())
}

async fn print_startup_diagnostics(client: &Client, namespace: &str) {
    info!("Kubeconfig found. Operator is running in '{}' namespace.", namespace);
    match client.apiserver_version().await {
        Ok(k8s_info) => {
            info!(r#"Kubernetes Api server info:
- Version: {}.{}
- Platform: {}
- Build date: {}"#,
                  k8s_info.major, k8s_info.minor, k8s_info.platform, k8s_info.build_date);
        }
        Err(error) => {
            error!("Unable to obtain details about Kubernetes cluster. Error:\n{}", error);
        }
    }
}

async fn create_mandatory_resources(client: Client, namespace: &str){
    let assisted_clustering_jar_var: String = std::env::var(configmap::H2O_CLUSTERING_JAR_PATH_KEY)
        .expect(&format!("H2O Clustering module JAR path environment variable '{}' not present. Search in current context folder failed.\
                This is most likely caused by misconfigured environment/docker image this operator is running in.", configmap::H2O_CLUSTERING_JAR_PATH_KEY));

    let clustering_module_path_buf: PathBuf = PathBuf::from_str(&assisted_clustering_jar_var)
        .expect(&format!("'{}' is not a valid path to H2O assisted clustering module jar.", &assisted_clustering_jar_var));
    let clustering_module_path : &Path = clustering_module_path_buf.as_path();

    if !clustering_module_path.is_file(){
        panic!("Path leading to H2O assisted clustering module JAR {} does not represent a file.", &assisted_clustering_jar_var);
    }

    if configmap::exists(client.clone(), namespace).await {
        info!("Existing configmap with H2O assisted clustering module found, attempting to delete.");
        match configmap::delete(client.clone(), namespace).await {
            Ok(_) => {
                info!("Existing configmap with H2O assisted clustering module successfully deleted.");
            }
            Err(error) => {
                panic!("Unable to delete existing configmap with H2O assisted clustering module. Error:\n{}", error)
            }
        }
    }

    let configmap_result: Result<ConfigMap, Error> = configmap::create_clustering_configmap(client, namespace, clustering_module_path).await;

    match configmap_result {
        Ok(configmap) => {
            info!("Successfully created ConfigMap '{}' with H2O assisted clustering module jar.", configmap.name());
        }
        Err(error) => {
            error!("Unable to create ConfigMap with H2O assisted clustering module jar. Make sure there are enough permissions inside the cluster. Error:\n{}", error);
            std::process::exit(1);
        }
    }

}

/// Initializes a possibly changing implementation of the [log](https://crates.io/crates/log) crate,
/// which acts as a facade.
///
/// Default logging level is set to `INFO`.
///
/// # Panics
/// Guaranteed to `panic!` when the logger implementation is unable to be initialized, for any reason,
/// as running the operator without logging is not desirable.
///
/// # Examples
///
/// ```no_run
/// initialize_logging();
/// ```
fn initialize_logging() {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)//TODO: User - Controllable logging
        .init()
        .unwrap();
}

#[cfg(test)]
mod tests{
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::process::{Child, Command};

    use futures::StreamExt;
    use k8s_openapi::api::core::v1::{Pod, Service};
    use kube::{Api, Client, Error};
    use kube::api::{DeleteParams, ListParams, Meta, PostParams, WatchEvent};
    use log::info;
    use crate::deployment::crd::{Resources, H2OSpec, H2O};
    use crate::deployment::{crd, configmap, client};

    #[tokio::test]
    async fn test_operator_deploy_undeploy() {
        let kubeconfig_location: PathBuf = tests_common::kubeconfig_location_panic();
        let (client, namespace): (Client, String) = client::try_default().await.unwrap();

        let mut h2o_operator_process: Child = start_h2o_operator(kubeconfig_location.to_str().unwrap());

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

        crd::wait_ready_h2o(client.clone(), h2o_name, &namespace).await;

        // Check the service has been created as well
        let service_api: Api<Service> = Api::namespaced(client.clone(), &namespace);
        let service: Service = service_api.get(h2o_name).await.unwrap();
        assert!(service.spec.unwrap().cluster_ip.is_some());

        h2o_api.delete(h2o_name, &DeleteParams::default()).await.unwrap();
        configmap::delete(client.clone(), &namespace).await.unwrap();
        assert!(wait_pods_deleted(client.clone(), h2o_name, &namespace).await.is_ok());
        crd::wait_deleted_h2o(client.clone(), h2o_name, &namespace).await;

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
}