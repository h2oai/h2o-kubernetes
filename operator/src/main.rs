extern crate deployment;
extern crate futures;
extern crate log;
extern crate simple_logger;

use std::time::Duration;

use kube::{Client, Error};
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;

use deployment::crd;

mod controller;

#[tokio::main]
async fn main() -> Result<(), Error> {
    init();
    let (client, namespace): (Client, String) = deployment::try_default().await?;
    info!("Kubeconfig found. Using default namespace: {}", &namespace);
    deploy_crd(client.clone()).await;
    controller::run(client.clone(), &namespace).await;

    Ok(())
}

fn init() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
}

async fn deploy_crd(client: Client) {
    if crd::exists(client.clone()).await {
        info!("Detected H2O CustomResourceDefinition already present in the cluster.");
    } else {
        info!("No H2O CustomResourceDefinition detected in the K8S cluster. Attempting to create it.");
        deployment::crd::create(client.clone()).await.unwrap();
        let timeout: Duration = Duration::from_secs(10);
        let result = deployment::crd::wait_crd_ready(client.clone(), timeout).await;
        match result {
            Ok(_) => {
                info!("Successfully deployed H2O CustomResourceDefinition into the cluster.");
            }
            Err(_) => {
                error!("H2O CustomResourceDefinition not accepted in {} seconds.", timeout.as_secs());
                std::process::exit(1);
            }
        }
    }
}
