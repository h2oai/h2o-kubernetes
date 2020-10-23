extern crate deployment;
extern crate futures;
extern crate log;
extern crate simple_logger;

use std::time::Duration;

use futures::StreamExt;
use kube::{Api, api::ListParams, Client, Error};
use kube_runtime::Controller;
use kube_runtime::controller::{Context, ReconcilerAction};
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;

use deployment::crd::H2O;
use deployment::DeploymentSpecification;

struct Data {
    client: Client,
    namespace: String,
}

impl Data {
    pub fn new(client: Client, namespace: String) -> Self {
        Data { client, namespace }
    }
}

fn init() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    init();
    let client: Client = Client::try_default().await.unwrap();
    let deployed = deployment::crd::is_deployed(client.clone()).await;
    info!("H2O CRD already found: {}", deployed);

    if !deployed {
        tokio::join!(deployment::crd::deploy(client.clone()));
    }

    let api: Api<H2O> = Api::all(client.clone());

    Controller::new(api.clone(), ListParams::default())
        .owns(api, ListParams::default())
        .run(reconcile, error_policy, Context::new(Data::new(client.clone(), "default".to_string())))
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("Reconciled {:?}", o),
                Err(e) => info!("Reconcile failed: {}", e),
            };
        }).await;

    Ok(())
}

async fn reconcile(h2o: H2O, context: Context<Data>) -> Result<ReconcilerAction, Error> {
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

fn error_policy(error: &Error, context: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(30)),
    }
}