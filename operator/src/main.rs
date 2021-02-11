extern crate deployment;
extern crate futures;
extern crate log;
extern crate simple_logger;
extern crate tokio;

use kube::Client;
use log::{info, error, LevelFilter};
use simple_logger::SimpleLogger;

use deployment::Error;
use deployment::configmap;
use std::path::{PathBuf, Path};
use std::str::FromStr;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::Meta;
use kube::client::Status;

mod controller;
mod clustering;
mod verification;

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
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();
}
