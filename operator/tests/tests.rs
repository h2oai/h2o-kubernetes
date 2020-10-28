extern crate deployment;
extern crate tests_common;

use std::process::{Child, Command};

use kube::{Api, Client};
use kube::api::{PostParams, DeleteParams};
use log::info;
use tokio::time::Duration;

use deployment::crd::{H2O, H2OSpec, Resources};

fn start_h2o_operator(kubeconfig_location: &str) -> Child {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("h2o-operator"));
    cmd.env("KUBECONFIG", kubeconfig_location);
    return cmd.spawn().unwrap();
}

#[tokio::test]
async fn test_deploy() {
    let kubeconfig_location = tests_common::kubeconfig_location_panic();
    let mut h2o_operator_process: Child = start_h2o_operator(kubeconfig_location.to_str().unwrap());
    let (client, namespace): (Client, String) = deployment::try_default().unwrap();
    let api: Api<H2O> = Api::namespaced(client.clone(), &namespace);

    let resources: Resources = Resources::new(1, "256Mi".to_string(), Option::None);
    deployment::crd::wait_ready(client.clone(), Duration::from_secs(180)).await.expect("CRD not available within timeout.");
    let h2o_spec: H2OSpec = H2OSpec::new(3, resources);
    let h2o: H2O = H2O::new("test", h2o_spec);
    let deployed_h2o: H2O = api.create(&PostParams::default(), &h2o).await.unwrap();

    api.delete("test", &DeleteParams::default()).await;

    h2o_operator_process.kill().unwrap();
}