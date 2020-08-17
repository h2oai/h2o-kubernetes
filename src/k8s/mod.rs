extern crate futures;
extern crate kube;

use std::path::{Path, PathBuf};

use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::extensions::v1beta1::Ingress;
use kube::Client;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

use self::futures::executor::block_on;
use self::kube::{Api, Config, Error};
use self::kube::api::{DeleteParams, Meta, PostParams};
use self::kube::config::{Kubeconfig, KubeConfigOptions};

mod definitions;

pub fn from_kubeconfig(kubeconfig_path: &Path) -> Client {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path).unwrap();
    let config: Config = block_on(Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())).unwrap();
    let client = Client::new(config);
    return client;
}

pub fn try_default() -> Result<Client, Error> {
    if let Some(openshift_kubeconfig_path) = find_openshift_kubeconfig() {
        return Ok(from_kubeconfig(openshift_kubeconfig_path.as_path()));
    } else {
        block_on(Client::try_default())
    }
}

fn find_openshift_kubeconfig() -> Option<PathBuf> {
    match dirs::home_dir() {
        None => { Option::None }
        Some(mut path) => {
            // OpenShift default config location in user's home folder. This is there `oc` tool saves kubeconfig after `oc login`.
            path.push(".kube/config");
            return if !path.exists() {
                Option::None
            } else {
                Some(path)
            };
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Deployment {
    pub specification: DeploymentSpecification,
    pub ingresses: Vec<Ingress>,
    pub stateful_sets: Vec<StatefulSet>,
    pub services: Vec<Service>,
}

impl Deployment {
    pub fn new(specification: DeploymentSpecification) -> Self {
        Deployment { specification, services: vec!(), ingresses: vec!(), stateful_sets: vec!() }
    }
}

/// Deployment as described by the user. No value is mandatory, as the set of required user inputs
/// and defaults might change over time and the dependent layers are forced to make no assumptions about which values
/// are actually present.
#[derive(Serialize, Deserialize, Debug)]
pub struct DeploymentSpecification {
    pub name: String,
    pub namespace: String,
    pub memory_percentage: u8,
    pub memory: String,
    pub num_cpu: u32,
    pub num_h2o_nodes: u32,
    pub kubeconfig_path: Option<PathBuf>,
}

impl DeploymentSpecification {
    pub fn new(name: String, namespace: String, memory_percentage: u8, memory: String, num_cpu: u32, num_h2o_nodes: u32, kubeconfig_path: Option<PathBuf>) -> Self {
        DeploymentSpecification { name, namespace, memory_percentage, memory, num_cpu, num_h2o_nodes, kubeconfig_path }
    }
}

pub fn deploy_h2o_cluster(client: &Client, deployment_specification: DeploymentSpecification) -> Deployment {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let mut deployment: Deployment = Deployment::new(deployment_specification);

    deploy_service(&mut tokio_runtime, client, &mut deployment);
    deploy_statefulset(&mut tokio_runtime, client, &mut deployment);

    return deployment;
}

#[inline]
fn deploy_service(tokio_runtime: &mut Runtime, client: &Client, deployment: &mut Deployment) {
    let service_api: Api<Service> = Api::namespaced(client.clone(), &deployment.specification.namespace);

    let service: Service = definitions::h2o_service(&deployment.specification.name, &deployment.specification.namespace);
    match tokio_runtime.block_on(service_api.create(&PostParams::default(), &service)) {
        Ok(service) => {
            deployment.services.push(service);
        }
        Err(e) => {
            eprintln!("Unable to deploy service for '{}' deployment. Rewinding existing deployment. Reason:\n{:?}", &deployment.specification.name, e);
            undeploy_h2o(&client, &deployment).unwrap();
            std::process::exit(1);
        }
    };
}

#[inline]
fn deploy_statefulset(tokio_runtime: &mut Runtime, client: &Client, deployment: &mut Deployment) {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), &deployment.specification.namespace);
    let stateful_set: StatefulSet = definitions::h2o_stateful_set(&deployment.specification.name, &deployment.specification.namespace, "h2oai/h2o-open-source-k8s", "latest",
                                                                  deployment.specification.num_h2o_nodes, deployment.specification.memory_percentage, &deployment.specification.memory, deployment.specification.num_cpu);
    match tokio_runtime.block_on(statefulset_api.create(&PostParams::default(), &stateful_set)) {
        Ok(statefulset) => { deployment.stateful_sets.push(statefulset); }
        Err(e) => {
            eprintln!("Unable to statefulset for '{}' deployment. Rewinding existing deployment. Reason:\n{:?}", &deployment.specification.name, e);
            undeploy_h2o(&client, &deployment).unwrap();
            std::process::exit(1);
        }
    }
}

pub fn undeploy_h2o(client: &Client, deployment: &Deployment) -> Result<(), Vec<String>> {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let namespace: &str = deployment.specification.namespace.as_str();
    let mut not_deleted: Vec<String> = vec!();

    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    for ingress in deployment.ingresses.iter() {
        match tokio_runtime.block_on(api.delete(ingress.name().as_str(), &DeleteParams::default())) {
            Ok(_) => {}
            Err(e) => { not_deleted.push(format!("Unable to undeploy '{}'. Reason:\n{:?}", ingress.name(), e)) }
        }
    }

    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    for service in deployment.services.iter() {
        match tokio_runtime.block_on(api.delete(service.name().as_str(), &DeleteParams::default())) {
            Ok(_) => {}
            Err(_) => { not_deleted.push(service.name()) }
        }
    }

    let api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);

    for stateful_set in deployment.stateful_sets.iter() {
        match tokio_runtime.block_on(api.delete(stateful_set.name().as_str(), &DeleteParams::default())) {
            Ok(_) => {}
            Err(_) => { not_deleted.push(stateful_set.name()) }
        }
    }

    return if not_deleted.len() > 0 {
        Err(not_deleted)
    } else {
        Ok(())
    };
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::k8s::{Deployment, DeploymentSpecification};
    use crate::tests::{kubeconfig_location_panic, TEST_CLUSTER_NAMESPACE};

    use super::kube::Client;

    #[test]
    fn test_from_kubeconfig() {
        let kubeconfig_location: String = kubeconfig_location_panic();
        let kubeconfig_path: &Path = Path::new(&kubeconfig_location);
        assert!(kubeconfig_path.exists());
        super::from_kubeconfig(kubeconfig_path);
    }

    #[test]
    fn test_deploy_h2o() {
        let kubeconfig_location: String = kubeconfig_location_panic();
        let kubeconfig_path: &Path = Path::new(&kubeconfig_location);
        assert!(kubeconfig_path.exists());
        let client: Client = super::from_kubeconfig(kubeconfig_path);
        let deployment_specification: DeploymentSpecification = DeploymentSpecification::new("h2o-k8s-test-cluster".to_string(), TEST_CLUSTER_NAMESPACE.to_string(),
                                                                                               80, "256Mi".to_string(), 2, 2, None);
        let deployment: Deployment = super::deploy_h2o_cluster(&client, deployment_specification);
        assert_eq!(1, deployment.services.len());
        assert_eq!(1, deployment.stateful_sets.len());
        assert_eq!(0, deployment.ingresses.len());
        let undeployment_result = super::undeploy_h2o(&client, &deployment);
        assert!(undeployment_result.is_ok());
    }
}
