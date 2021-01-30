extern crate deployment;
extern crate futures;
extern crate log;
extern crate simple_logger;
extern crate tokio;

use kube::Client;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;

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
