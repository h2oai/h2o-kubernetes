extern crate clap;
extern crate deployment;
extern crate tokio;

use k8s_openapi::api::networking::v1beta1::Ingress;
use kube::Client;

use cli::{Command, UserNewClusterSpecification};
use deployment::crd::{CustomImage, H2OSpec, Resources};
use deployment::Error;

use crate::cli::UserExistingClusterSpecification;

mod cli;

/// Entrypoint to H2O Kubernetes CLI binary. Created to aid deployment of H2O Open Source Machine Learning
/// platform to Kubernetes. For users, the number one choice should always be the H2O operator, as
/// a custom controller running inside the Kubernetes cluster is more powerful than any client-based
/// application. One of the functions of this CLI is the ability to deploy the operator into the Kubernetes cluster.
///
/// When the H2O operator can not be used for any reasons, including permissions or cluster limitations,
/// this CLI is able to create H2O clusters in Kubernetes as well, using mostly the same resources an operator would do,
/// without the additional benefit of overseeing the cluster the operator provides.
///
/// # Asynchronous execution
///
/// Nearly every Kubernetes operation (API request) is fulfilled asynchronously. This naturally translates
/// to its client libraries, including the [Kube crate](https://github.com/clux/kube-rs), which leverages
/// the Rust's [async](https://rust-lang.github.io/async-book/) capabilities. Naturally, the resource management
/// done by this CLI can be mostly done in an asynchronous way, preferably in parallel. Therefore, asynchronous
/// functions are at the very core of this CLI.
///
/// In Rust, the asynchronous code is runtime-agnostic, this means the user might pick any runtime available.
/// As the `kube` crate already uses a commonly used crate named [Tokio](https://tokio.rs/), the choice
/// is to Tokio as well. It's multi-threaded executor is chosen for maximum performance and parallelism, even though
/// not really needed at the moment. This is marked by choosing the `rt-threaded` feature in this crate's `Cargo.tml`.
#[tokio::main]
async fn main() {
    let command: Command = match cli::get_command() {
        Ok(cmd) => cmd,
        Err(error) => {
            eprintln!("Unable to process user input: {:?}", error);
            std::process::exit(1);
        }
    };
    match command {
        Command::CreateCluster(new_deployment) => {
            create_new_deployment(new_deployment).await;
        }
        Command::DeleteCluster(existing_deployment_spec) => {
            delete_existing_deployment(existing_deployment_spec).await;
        }
        Command::Ingress(existing_deployment_spec) => {
            add_ingress(existing_deployment_spec).await;
        }
    };
}

/// Creates a new H2O cluster in a Kubernetes cluster. Deploys all the resources necessary
/// for H2O to form a cluster.
///
/// # Arguments
///
/// `user_spec` - Specification of the deployment parameters -
async fn create_new_deployment(user_spec: UserNewClusterSpecification) {
    let (client, namespace): (Client, String) = match user_spec.kubeconfig_path {
        None => deployment::client::try_default().await.unwrap(),
        Some(kubeconfig_path) => deployment::client::from_kubeconfig(kubeconfig_path.as_path()).await
            .unwrap(),
    };

    let resources: Resources = Resources::new(
        user_spec.num_cpu,
        user_spec.memory,
        Some(user_spec.memory_percentage),
    );
    let custom_image: Option<CustomImage> = match user_spec.custom_image {
        None => Option::None,
        Some(img) => Option::Some(CustomImage::new(img, user_spec.custom_command)),
    };
    let specification: H2OSpec = H2OSpec::new(
        user_spec.num_h2o_nodes,
        user_spec.version,
        resources,
        custom_image,
    );
    match deployment::create_h2o_cluster(
        client.clone(),
        &specification,
        &namespace,
        &user_spec.name,
    )
        .await
    {
        Ok(successful_deployment) => successful_deployment,
        Err(error) => {
            panic!("Unable to deploy H2O cluster. Error:\n{:?}", error);
        }
    };

    println!(
        "Deployment of '{}' completed successfully.",
        &user_spec.name
    );
    println!(
        "To undeploy, use the 'h2ok undeploy {}' command.",
        &user_spec.name
    );
}

/// Deletes an existing deployment and all its sub-resources. The deletion is asynchronous -
/// and the resources might be deleted in parallel. This method does not wait for the deletion process to be completed, as
/// this is the responsibility of the respective controllers.
///
/// # Arguments
/// `specification` - A descriptor of an existing deployment to delete.
async fn delete_existing_deployment(specification: UserExistingClusterSpecification) {
    let (client, namespace): (Client, String) = match specification.kubeconfig_path {
        None => deployment::client::try_default().await.unwrap(),
        Some(kubeconfig_path) => deployment::client::from_kubeconfig(kubeconfig_path.as_path()).await
            .unwrap(),
    };

    let deletion_result: Result<(), Error> = deployment::delete_h2o_cluster(
        client.clone(),
        &specification.namespace.unwrap_or(namespace),
        &specification.name,
    )
        .await;

    match deletion_result {
        Ok(_) => {}
        Err(error) => {
            print!(
                "Unable to undeploy H2O named '{}'. Error:\n{:?}",
                &specification.name, error
            );
        }
    }
    println!("Removed deployment '{}'.", &specification.name);
}

/// Adds an ingress to an existing deployment specification. The ingress is pointed to the
/// headless service used for H2O node discovery, as when the H2O cluster is ready, only one the
/// pod with the H2O Leader node passes the readiness probe.
///
/// Asynchronous method. The ingress might not yet be fully initialized when this method returns.
///
/// # Arguments
/// `specification` - A descriptor of an existing deployment to point ingress to.
async fn add_ingress(specification: UserExistingClusterSpecification) {
    let (client, namespace): (Client, String) = match specification.kubeconfig_path {
        None => deployment::client::try_default().await.unwrap(),
        Some(kubeconfig_path) => deployment::client::from_kubeconfig(kubeconfig_path.as_path()).await
            .unwrap(),
    };

    let ingress: Ingress = deployment::ingress::create(
        client.clone(),
        &specification.namespace.unwrap_or(namespace),
        &specification.name,
    )
        .await.expect(&format!(
        "Unable to create ingress for {} deployment.", specification.name));

    println!("Ingress '{}' deployed successfully.", &specification.name);
    let ingress_ip: Option<String> = deployment::ingress::any_lb_external_ip(&ingress);
    let ingress_path: Option<String> = deployment::ingress::any_path(&ingress);

    if ingress_ip.is_some() && ingress_path.is_some() {
        println!("You may now use 'h2o.connect()' to connect to the H2O cluster:");
        println!(
            "Python: 'h2o.connect(url=\"http://{}:80{}\")'",
            ingress_ip.as_ref().unwrap(),
            ingress_path.as_ref().unwrap()
        );
        println!(
            "R: 'h2o.connect(ip = \"{}\", context_path = \"{}\", port=80)'",
            ingress_ip.as_ref().unwrap(),
            ingress_path.unwrap().strip_prefix("/").unwrap()
        )
    }
}
