use kube_runtime::Controller;
use kube::api::ListParams;
use kube::{Client, Error, Api};
use kube_runtime::controller::{ReconcilerAction, Context};
use deployment::DeploymentSpecification;
use std::time::Duration;
use futures::StreamExt;
use deployment::crd::H2O;

use log::info;

pub async fn run(client: Client, deployment_namespace: &str){
    let api: Api<H2O> = Api::namespaced(client.clone(), deployment_namespace);
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

async fn reconcile(h2o: H2O, context: Context<Data>) -> Result<ReconcilerAction, Error> {
    println!("{:?}", h2o);
    let data: &Data = context.get_ref();
    let nodes: u32 = h2o.spec.nodes;
    let name: String = h2o.metadata.name.unwrap();
    let memory_percentage: u8 = h2o.spec.resources.memory_percentage.unwrap_or(50);
    let memory: String = h2o.spec.resources.memory;
    let cpu: u32 = h2o.spec.resources.cpu;

    let deployment_spec: DeploymentSpecification = DeploymentSpecification::new(name, data.namespace.clone(), memory_percentage, memory, cpu, nodes, Option::None);
    deployment::deploy_h2o_cluster(data.client.clone(), deployment_spec).await?;

    return Ok(ReconcilerAction {
        requeue_after: None
    });
}

fn error_policy(_error: &Error, _context: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(30)),
    }
}