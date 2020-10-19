use std::time::Duration;

use futures::executor::block_on;
use futures::StreamExt;
use kube::{api::{ListParams}, Api, Client, CustomResource, Error};
use kube_runtime::Controller;
use kube_runtime::controller::{Context, ReconcilerAction};
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize)]
#[kube(group = "h2o.ai", version = "v1", kind = "H2O")]
#[kube(shortname = "h2o", namespaced)]
struct H2OSpec {
    nodes: u32
}

struct Data {
    client: Client,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let client: Client = block_on(Client::try_default()).unwrap();
    let api: Api<H2O> = Api::all(client.clone());

    Controller::new(api.clone(), ListParams::default())
        .owns(api, ListParams::default())
        .run(reconcile, error_policy, Context::new(Data { client: client.clone() }))
        .for_each(|res| async move {
            match res {
                Ok(o) => println!("reconciled {:?}", o),
                Err(e) => println!("reconcile failed: {}", e),
            };
        }).await;

    Ok(())
}

async fn reconcile(h2o: H2O, context: Context<Data>) -> Result<ReconcilerAction, Error> {
    return Ok(ReconcilerAction {
        requeue_after: Some(Duration::from_secs(300))
    });
}

fn error_policy(_error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(10)),
    }
}