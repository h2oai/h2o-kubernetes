extern crate futures;
extern crate kube;

use std::path::Path;

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

pub fn deploy_h2o(client: &Client, name: &str, namespace: &str, nodes: i32) -> Deployment {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let ingress_api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let ingress = definitions::h2o_ingress(name, namespace);
    let ingress = tokio_runtime.block_on(ingress_api.create(&PostParams::default(), &ingress)).unwrap();

    let service_api: Api<_> = Api::namespaced(client.clone(), namespace);

    let service: Service = definitions::h2o_service(name, namespace);
    let service: Service = tokio_runtime.block_on(service_api.create(&PostParams::default(), &service)).unwrap();

    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let stateful_set: StatefulSet = definitions::h2o_stateful_set(name, namespace, "h2oai/h2o-open-source-k8s", "latest", nodes);
    let stateful_set: StatefulSet = tokio_runtime.block_on(statefulset_api.create(&PostParams::default(), &stateful_set)).unwrap();
    return Deployment::new(String::from(name), String::from(namespace), Option::None,
                           vec!(ingress), vec!(stateful_set), vec!(service));
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
        let deployment: Deployment = super::deploy_h2o(&client, "h2o-k8s-test-cluster", TEST_CLUSTER_NAMESPACE, 2);
        let undeployment_result = super::undeploy_h2o(&client, &deployment);
        assert!(undeployment_result.is_ok());
    }
}
