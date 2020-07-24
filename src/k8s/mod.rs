extern crate futures;
extern crate kube;

use std::path::{Path};

use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::extensions::v1beta1::Ingress;
use kube::Client;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

use self::futures::executor::block_on;
use self::kube::{Api, Config};
use self::kube::api::{DeleteParams, Meta, PostParams};
use self::kube::config::{Kubeconfig, KubeConfigOptions};

mod definitions;

pub fn from_kubeconfig(kubeconfig_path: &Path) -> Client {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path).unwrap();
    let config: Config = block_on(Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())).unwrap();
    let client = Client::new(config);
    return client;
}

pub fn try_default() -> Client {
    return block_on(Client::try_default()).unwrap();
}

pub fn try_openshift_kubeconfig() -> (Option<String>, Option<Client>) {
    match dirs::home_dir() {
        None => { return (Option::None, Option::None) }
        Some(mut path) => {
            // OpenShift default config location in user's home folder. This is there `oc` tool saves kubeconfig after `oc login`.
            path.push(".kube/config");
            return if !path.exists() {
                (Option::None, Option::None)
            } else {
                (Some(String::from(path.to_str().unwrap())), Some(from_kubeconfig(path.as_path())))
            };
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Deployment {
    pub name: String,
    pub namespace: String,
    pub kubeconfig_path: Option<String>,
    pub ingresses: Vec<Ingress>,
    pub stateful_sets: Vec<StatefulSet>,
    pub services: Vec<Service>,
}

impl Deployment {
    fn new(name: String, namespace: String, kubeconfig_path: Option<String>, ingresses: Vec<Ingress>,
           stateful_sets: Vec<StatefulSet>, services: Vec<Service>) -> Deployment {
        return Deployment {
            name: name,
            namespace: namespace,
            kubeconfig_path: kubeconfig_path,
            ingresses: ingresses,
            stateful_sets: stateful_sets,
            services: services,
        };
    }
}

pub fn deploy_h2o(client: &Client, deployment_name: &str, namespace: &str, nodes: i32, memory_percentage: u8, memory: &str,
                  num_cpu: u32) -> Deployment {
    let mut deployment: Deployment = Deployment::new(String::from(deployment_name), String::from(namespace), Option::None,
                                                     vec!(), vec!(), vec!());
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let ingress_api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let ingress = definitions::h2o_ingress(deployment_name, namespace);
    match tokio_runtime.block_on(ingress_api.create(&PostParams::default(), &ingress)) {
        Ok(ingress) => { deployment.ingresses.push(ingress); }
        Err(e) => {
            eprintln!("Unable to deploy ingress for '{}' deployment. Rewinding existing deployment. Reason: \n{:?}", deployment_name, e);
            undeploy_h2o(client, &deployment).unwrap();
            std::process::exit(1);
        }
    }


    let service_api: Api<_> = Api::namespaced(client.clone(), namespace);

    let service: Service = definitions::h2o_service(deployment_name, namespace);
    match tokio_runtime.block_on(service_api.create(&PostParams::default(), &service)) {
        Ok(service) => { deployment.services.push(service); }
        Err(e) => {
            eprintln!("Unable to deploy service for '{}' deployment. Rewinding existing deployment. Reason: \n{:?}", deployment_name, e);
            undeploy_h2o(client, &deployment).unwrap();
            std::process::exit(1);
        }
    }

    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let stateful_set: StatefulSet = definitions::h2o_stateful_set(deployment_name, namespace, "h2oai/h2o-open-source-k8s", "latest",
                                                                  nodes, memory_percentage, memory, num_cpu);
    match tokio_runtime.block_on(statefulset_api.create(&PostParams::default(), &stateful_set)) {
        Ok(statefulset) => { deployment.stateful_sets.push(statefulset); }
        Err(e) => {
            eprintln!("Unable to deploy service for '{}' deployment. Rewinding existing deployment. Reason: \n{:?}", deployment_name, e);
            undeploy_h2o(client, &deployment).unwrap();
            std::process::exit(1);
        }
    }

    return deployment;
}

pub fn undeploy_h2o(client: &Client, deployment: &Deployment) -> Result<(), Vec<String>> {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let namespace: &str = deployment.namespace.as_str();
    let mut not_deleted: Vec<String> = vec!();

    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    for ingress in deployment.ingresses.iter() {
        match tokio_runtime.block_on(api.delete(ingress.name().as_str(), &DeleteParams::default())) {
            Ok(_) => {}
            Err(_) => { not_deleted.push(ingress.name()) }
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

    use crate::k8s::Deployment;
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
        let deployment: Deployment = super::deploy_h2o(&client, "h2o-k8s-test-cluster", TEST_CLUSTER_NAMESPACE, 2, 50, "4Gi", 1);
        let undeployment_result = super::undeploy_h2o(&client, &deployment);
        assert!(undeployment_result.is_ok());
    }
}
