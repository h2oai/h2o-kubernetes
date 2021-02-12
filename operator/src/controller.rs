use std::time::Duration;

use futures::StreamExt;
use kube::{Api, Client};
use kube::api::{ListParams, Meta};
use kube_runtime::controller::{Context, ReconcilerAction};
use kube_runtime::Controller;
use log::{error, info};

use deployment::crd::{H2O, H2OSpec};
use deployment::Error;
use crate::clustering;
use k8s_openapi::api::core::v1::Pod;

/// Creates and runs an instance of `kube_runtime::Controller` internally, endlessly waiting for incoming events
/// on CRDs handled by this operator. Unless there is an error, this function never returns.
///
/// On top level, the incoming CRD event is either successfully reconciled or re-scheduled for later reconciliation,
/// while an error is logged.
///
/// # Controller lifecycle & downtime
/// If either the controller itself is not running, or the whole operator binary is down/unreachable,
/// they're not lost. Once the operator is up and running again, the event queue is started to be reconciled once again.
///
/// One special case is resource deletion, e.g. `kubectl delete h2o some-h2o`. Before the actual `H2O` CRD
/// is deleted, deletion of various others resources must be invoked before hand. For example Statefulsets with H2O pods
/// or the headless service for H2O node discovery.
///
/// Kubernetes itself would delete the `H2O` resource immediately without a `finalizer` record present.
/// Finalizer, as defined by [Kubernetes documentation](https://book.kubebuilder.io/reference/using-finalizers.html)
/// serves as an asynchronous pre-delete hook. In practice, if there is a finalizer record on a resource,
/// it is not deleted unless removed by the controller. Therefore, if `kubectl delete` is invoked on any
/// `H2O` resource, it won't be deleted unless a running H2O operator/controller removes it. The removal
/// itself is done when `delete` operation on all underlying resources is invoked.
///
/// The removal of resources is, as almost everything in Kubernetes, done asynchronously. As soon as
/// statefulsets, services and other resources are instructed to be deleted, it is the duty of the respective
/// controllers to delete them and the finalizer record is removed, allowing for the `H2O` resource to be removed.
///
/// # Arguments
/// - `client` - A Kubernetes client from the `kube` crate. Required to create other resources representing the
/// final H2O cluster in Kubernetes.
/// - `namespace` - H2O operator is namespace-scoped. H2Os are deployed into the namespace the operator has been deployed to.
///
/// # Examples
///
/// ```no_run
///     let (client, namespace): (Client, String) = deployment::try_default().await?;
///     controller::run(client, &namespace).await;
/// ```
pub async fn run(client: Client, namespace: &str) {
    let api: Api<H2O> = Api::namespaced(client.clone(), namespace);
    Controller::new(api.clone(), ListParams::default())
        .owns(api, ListParams::default())
        .run(
            reconcile,
            error_policy,
            Context::new(ContextData::new(client.clone(), namespace.to_string())),
        )
        .for_each(|res| async move {
            match res {
                Ok(_) => {},
                Err(err) => info!("Failed to reconcile: {}", err),
            };
        })
        .await;
}

/// Context data inserted into the reconciliation handler with each call.
struct ContextData {
    /// Kubernetes client to manipulate Kubernetes resources
    client: Client,
    /// Namespace to deploy H2O subresources to. Also the namespace this operator has been deployed to.
    namespace: String,
}

impl ContextData {
    /// Creates a new instance of `ContextData`.
    ///
    /// # Arguments
    ///
    /// - `client` - Kubernetes client to manipulate Kubernetes resources
    /// - `default_namespace` - Default namespace to deploy resources to - unless explicitly specified by the user
    pub fn new(client: Client, default_namespace: String) -> Self {
        ContextData { client, namespace: default_namespace }
    }
}

/// Action to be taken by the controller if there is a new event on `H2O` resource.
enum ControllerAction {
    /// Create a new H2O cluster
    Create,
    /// Delete resources of an existing H2O Cluster
    Delete,
    /// Updating existing H2O deployment is not supported - once H2O is clustered, it is immutable. Any events requiring on actions.
    Verify,
}

/// Reconciliation logic router, called by the controller once per each event.
/// See `ControllerAction` for details on routing logic.s
///
/// # Arguments
/// `h2o` - The `H2O` resource, constructed automatically by the controller.
/// `context` - Context to be injected into each reconciliation. Injected automatically by the controller.
///
/// # Examples
/// No examples provided, as this method should be called only by the controller.
async fn reconcile(h2o: H2O, context: Context<ContextData>) -> Result<ReconcilerAction, Error> {
    match examine_h2o_for_actions(&h2o) {
        ControllerAction::Create => {
            create_h2o_deployment(&h2o, &context).await?;
        }
        ControllerAction::Delete => {
            delete_h2o_deployment(&h2o, &context).await?;
        }
        ControllerAction::Verify => {
            info!("Verifying an existing H2O deployment '{}'", h2o.name()); // Log the whole incoming H2O description
            let h2o_serialized: String = serde_yaml::to_string(&h2o).unwrap_or(h2o.name());
            info!("H2O '{}' verified. Status OK. ", h2o.name()); // Log the whole incoming H2O description
        }
    }

    return Ok(ReconcilerAction {
        requeue_after: Some(Duration::from_secs(5)),
    });
}

/// Reconciliation failure logic, intended to be called by the controller itself. Logs the error
/// causing the failure on `error` level and re-schedules the event for later reconciliation.
///
/// # Arguments
/// `error` - The cause of reconciliation failure
/// `_context` - An instance of `ContextData`, provided by the controller with each reconciliation event.
///
///# Examples
/// As this function is intended to be called by the controller only, there are no examples.
fn error_policy(error: &Error, _context: Context<ContextData>) -> ReconcilerAction {
    error!("Reconciliation error:\n{:?}", error);
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(5)),
    }
}

/// Examines the incoming `H2O` resource and determines the `ControllerAction` to be taken
/// upon it.
///
/// # Arguments
///
/// `h2o` - The `H2O` resource instance, representing the current state of the resource in Kubernetes cluster.
///
/// # Examples
/// As thins function is intended to be ran directly in the reconciliation loop of the controller and
/// and `H2O` instance is constructed by deserializing the H2O resource obtained from Kubernetes itself,
/// the usage is limited and therefore there are no examples.
fn examine_h2o_for_actions(h2o: &H2O) -> ControllerAction {
    let has_finalizer: bool = deployment::crd::has_h2o3_finalizer(&h2o);
    let has_deletion_timestamp: bool = deployment::crd::has_deletion_stamp(&h2o);
    return if has_finalizer && has_deletion_timestamp {
        ControllerAction::Delete
    } else if !has_finalizer && !has_deletion_timestamp {
        ControllerAction::Create
    } else {
        ControllerAction::Verify
    };
}

/// Creates an H2O deployment as dictated by the `H2O` resource specification obtained,
/// including but not limited to statefulsets, including its respective pods and headless services to make H2O
/// clustering possible. These sub-resources are built-in to each Kubernetes cluster and handled by their respective
/// controllers. The order of creation of the sub-resources is not guaranteed and is invoked asynchronously.
///
/// Creates an H2O-specific finalizer on the existing `H2O` resources to indicate pre-deletion hooks must
/// be handled by this operator before resource deletion.
///
/// # Arguments
/// `h2o` - The `H2O` resource instance, representing the current state of the resource in Kubernetes cluster.
/// `context` - An instance of `ContextData`, provided by the controller with each reconciliation event.
async fn create_h2o_deployment(
    h2o: &H2O,
    context: &Context<ContextData>,
) -> Result<ReconcilerAction, Error> {
    match serde_yaml::to_string(h2o) {
        Ok(h2o_yaml) => {
            info!("Attempting to create the following H2O cluster:\n{}", h2o_yaml)
        }
        Err(_) => {
            error!("Attempting to create H2O cluster: {}", h2o.name());
        }
    };
    info!("{}", serde_yaml::to_string(h2o).unwrap());
    let data: &ContextData = context.get_ref();
    let name: String = h2o.metadata.name.clone()
        .ok_or(Error::UserError("Unable to create H2O deployment. No H2O name provided.".to_string()))?;

    let create_pods_result = create_h2o_pods(data.client.clone(), &h2o.spec, &name, &data.namespace).await;

    match create_pods_result{
        Ok(_) => {
            clustering::cluster_pods(data.client.clone(), &data.namespace, &name, h2o.spec.nodes as usize).await;
            deployment::finalizer::add_finalizer(data.client.clone(), &data.namespace, &name).await.unwrap();
            deployment::crd::set_ready_condition(data.client.clone(), &name, &data.namespace, true).await.unwrap();
        }
        Err(_) => {
                return Err(Error::DeploymentError("".to_owned()));
        }
    }

    info!("H2O '{}' successfully deployed.", &name);
    return Ok(ReconcilerAction {
        requeue_after: Option::Some(Duration::from_secs(10)),
    });
}

async fn create_h2o_pods(client: Client, h2o_spec: &H2OSpec, name: &str, namespace: &str) -> Result<(), ()>{
    let pod_creation_result: Result<Vec<Pod>, Vec<Error>> = deployment::pod::create_pods(client, h2o_spec, name, namespace).await;
    match pod_creation_result {
        Ok(pods) => {
            let mut pods_ips: String = String::new();
            for pod in pods.iter(){
                let pod_ip: String = deployment::pod::get_pod_ip(pod);
                pods_ips.push_str(&pod_ip);
                pods_ips.push('\n');
            }

            info!("The following pods were created for '{}':\n{}", name, pods_ips);
            Ok(())
        }
        Err(errors) => {
            let errors_joined: String = errors.iter()
                .map(|err| err.to_string())
                .collect::<Vec<String>>()
                .join(",");
        error!("Unable to create pods for '{}'. Errors:\n{}", name, errors_joined);
            Err(())
        }
    }
}

/// Deletes all resources related to the given `H2O` resource intended for deletion,
/// including but not necessarily limited to statefulsets, including its respective pods and headless services.
/// It is assumed an H2o-specific finalizer is present on the resource before. The finalizer is removed from the `H2O` resource
/// right after deletion of all sub-resources is issued.
///
/// The sub-resources are deleted asynchronously and order of their deletion is not guaranteed, as
/// each sub-resource deletion is handled by its own controller. Therefore, naturally, once this method
/// returns, it is not a guarantee of the sub-resources being deleted. This is correct approach, as
/// resource management lifecycle is abstracted away in Kubernetes and layers should not block each other unless
/// necessary.
///
/// /// `h2o` - The `H2O` resource instance, representing the current state of the resource in Kubernetes cluster.
// /// `context` - An instance of `ContextData`, provided by the controller with each reconciliation event.
async fn delete_h2o_deployment(
    h2o: &H2O,
    context: &Context<ContextData>,
) -> Result<ReconcilerAction, Error> {
    info!("Attempting to delete H2O deployment: {}", h2o.name());
    let data: &ContextData = context.get_ref();
    let client: Client = data.client.clone();
    let name: &str = h2o.metadata.name.as_ref()
        .ok_or(Error::UserError("Unable to delete H2O deployment. No H2O name provided.".to_string()))?;
    let namespace: &str = h2o.meta().namespace.as_ref()
        .ok_or(Error::UserError("Unable to delete H2O deployment. No namespace provided.".to_string()))?;
    deployment::service::delete(client.clone(), namespace, name).await.unwrap();
    deployment::pod::delete_pods_label(client.clone(), namespace, name).await;
    deployment::pod::wait_pods_deleted(client.clone(), name, namespace).await?; // TODO: timeout

    // TODO: Wait for resources to be deleted before exit.

    deployment::finalizer::remove_finalizer(data.client.clone(), name, namespace).await?;

    info!("Deleted H2O '{}'.", &name);
    return Ok(ReconcilerAction {
        requeue_after: Option::None,
    });
}
