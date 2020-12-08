extern crate deployment;
extern crate futures;
extern crate log;
extern crate simple_logger;
extern crate tokio;

use std::collections::HashSet;
use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::Client;
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;

use deployment::crd;
use deployment::crd::CRDReadiness;
use deployment::Error;

mod controller;

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
    let (client, namespace): (Client, String) = deployment::client::try_default().await?;
    info!("Kubeconfig found. Using default namespace: {}", &namespace);
    ensure_crd_created(client.clone()).await;
    controller::run(client.clone(), &namespace).await;

    Ok(())
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

/// Ensures all of the following:
/// - H2O CRD is deployed
/// - The H2O CRD declares support for exactly the same `H2OSpec` versions as this operator.
///
/// If there is no CRD deployed yet, an attempt to deploy it is made.
/// The CRDs are created cluster-wide and not tied to a specific Kubernetes namespace. This is one possible source of
/// errors, as permissions of the user running the operator might not suffice.
///
/// If there is H2O CRD already present, this function will inspect the supported `H2OSpec` versions
/// and make sure the already present H2O CRD supports exactly the same `H2OSpec` versions as this operator.
/// Otherwise terminates the operator process with exit code `1`. The reason for this behavior is
/// the inability to reconcile unsupported versions of the H2O specifications. Users are required to
/// delete the old CRD first, as the CRD deletion process triggers `delete` operation on existing H2O deployments.
/// Those delete operations are delegated into the respective operator, which is obliged to delete the existing H2O deployments first.
/// Only after all existing H2O deployments are deleted, the H2O custom resource definition is finally deleted
/// and it is possible to deploy other version of the H2O CRD.
///
/// # Arguments
///
/// - `client` - A Kubernetes client from the `kube` crate - to make API queries and CRD creation possible.
///
/// # Examples
///
/// ```no_run
/// extern crate kube;
/// use kube::Client;
///
/// let client = Client::try_default();
/// ensure_crd_created(client).await;
///
/// # Panics
///
/// - If `H2O` CRD is not detected and creation fails, the operator would be running in vain.
/// - If `H2O` CRD is present and declares support for a different set of `H2OSpec`s versions.
/// ```
async fn ensure_crd_created(client: Client) {
    let existing_crd_result = crd::get_current(client.clone()).await;

    match existing_crd_result {
        Ok(existing_crd) => {
            info!("Detected H2O CustomResourceDefinition already present in the cluster.");
            let new_crd = crd::construct_h2o_crd()
                .expect("Unable to construct H2O CRD for version compatibility check.");
            let existing_crd_supported_versions: HashSet<&str> = crd::spec_versions(&existing_crd);
            let this_crd_supported_versions: HashSet<&str> = crd::spec_versions(&new_crd);

            if this_crd_supported_versions.eq(&existing_crd_supported_versions)
                && existing_crd_supported_versions.len() == this_crd_supported_versions.len() {
                info!("Existing H2O CRD supports the same specification versions as this operator: {:?}", &this_crd_supported_versions)
            } else {
                error!("Existing H2O CRD supports versions different from this operator:\n{:?}.\
                 \nThis operator supports:\n{:?}.\nExiting.", &existing_crd_supported_versions, &this_crd_supported_versions);
                std::process::exit(1);
            }
        }
        Err(_) => {
            info!(
                "No H2O CustomResourceDefinition detected in the K8S cluster. Attempting to create it."
            );
            let created_crd: CustomResourceDefinition = deployment::crd::create(client.clone()).await.unwrap();
            let timeout: Duration = Duration::from_secs(30);
            let result = deployment::crd::wait_crd_status(client.clone(), timeout, CRDReadiness::Ready).await;
            match result {
                Ok(_) => {
                    info!("Successfully deployed H2O CustomResourceDefinition into the cluster. Supported specification versions: {:?}", crd::spec_versions(&created_crd));
                }
                Err(error) => {
                    error!(
                        "H2O CustomResourceDefinition not accepted in {} seconds. Reason:\n{}",
                        timeout.as_secs(), error
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}
