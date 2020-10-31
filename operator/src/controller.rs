use std::time::Duration;

use futures::StreamExt;
use kube::{Api, Client, Error};
use kube::api::{ListParams, Meta, DeleteParams};
use kube_runtime::Controller;
use kube_runtime::controller::{Context, ReconcilerAction};
use log::info;

use deployment::{Deployment, DeploymentSpecification};
use deployment::crd::H2O;

pub async fn run(client: Client, deployment_namespace: &str) {
    let api: Api<H2O> = Api::all(client.clone());
    Controller::new(api.clone(), ListParams::default())
        .owns(api, ListParams::default())
        .run(reconcile, error_policy, Context::new(Data::new(client.clone(), deployment_namespace.to_string())))
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("Reconciled {:?}", o),
                Err(e) => info!("Reconcile failed: {}", e),
            };
        }).await;
}

struct Data {
    client: Client,
    namespace: String,
}

impl Data {
    pub fn new(client: Client, namespace: String) -> Self {
        Data { client, namespace }
    }
}

enum ControllerAction {
    Create,
    Delete,
    Noop, // Updating existing H2O deployment is not supported - once H2O is clustered, it is immutable
}

async fn reconcile(h2o: H2O, context: Context<Data>) -> Result<ReconcilerAction, Error> {
    match scan_h2o_for_actions(&h2o) {
        ControllerAction::Create => {
            create_h2o_deployment(&h2o, &context).await?;
        }
        ControllerAction::Delete => {
            delete_h2o_deployment(&h2o, &context).await?;
        }
        ControllerAction::Noop => {
            info!("No action taken for:\n{:?}", &h2o); // Log the whole incoming H2O description
        }
    }

    return Ok(ReconcilerAction {
        requeue_after: None
    });
}

fn error_policy(_error: &Error, _context: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(5)),
    }
}

fn scan_h2o_for_actions(h2o: &H2O) -> ControllerAction {
    let has_finalizer = has_h2o3_finalizer(&h2o);
    let has_deletion_timestamp = has_deletion_stamp(&h2o);
    return if has_finalizer && has_deletion_timestamp {
        ControllerAction::Delete
    } else if !has_finalizer && !has_deletion_timestamp {
        ControllerAction::Create
    } else {
        ControllerAction::Noop
    };
}

fn has_h2o3_finalizer(h2o: &H2O) -> bool {
    return match h2o.metadata.finalizers.as_ref() {
        Some(finalizers) => {
            finalizers.contains(&String::from(deployment::finalizer::FINALIZER_NAME))
        }
        None => { false }
    };
}

fn has_deletion_stamp(h2o: &H2O) -> bool {
    return h2o.metadata.deletion_timestamp.is_some();
}

async fn create_h2o_deployment(h2o: &H2O, context: &Context<Data>) -> Result<ReconcilerAction, Error> {
    let data: &Data = context.get_ref();
    let nodes: u32 = h2o.spec.nodes;
    let name: String = h2o.metadata.name.clone().unwrap();
    let memory_percentage: u8 = h2o.spec.resources.memory_percentage.unwrap_or(50);
    let memory: String = h2o.spec.resources.memory.clone();
    let cpu: u32 = h2o.spec.resources.cpu;

    let deployment_spec: DeploymentSpecification = DeploymentSpecification::new(name.clone(), data.namespace.clone(), memory_percentage, memory, cpu, nodes, Option::None);
    let deploy_future = deployment::deploy_h2o_cluster(data.client.clone(), deployment_spec);
    let add_finalizer_future = deployment::finalizer::add_finalizer(data.client.clone(), &name, &data.namespace);

    tokio::try_join!(deploy_future, add_finalizer_future)?;

    return Ok(ReconcilerAction {
        requeue_after: Option::None
    });
}

async fn delete_h2o_deployment(h2o: &H2O, context: &Context<Data>) -> Result<ReconcilerAction, Error> {
    let data: &Data = context.get_ref();
    let name: &str = h2o.metadata.name.as_ref().unwrap();
    let namespace: &str = h2o.meta().namespace.as_ref().unwrap();
    let statefulset_future = deployment::statefulset::delete(data.client.clone(), name, namespace);
    let service_future = deployment::service::delete(data.client.clone(), name, namespace);

    tokio::join!(statefulset_future, service_future); // todo: handle the errors

    deployment::finalizer::remove_finalizer(data.client.clone(), name, namespace).await?;

    return Ok(ReconcilerAction {
        requeue_after: Option::None
    });
}