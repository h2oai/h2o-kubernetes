extern crate futures;
extern crate kube;

use std::path::{Path, PathBuf};

use futures::{StreamExt, TryStreamExt};
use futures::executor::block_on;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1beta1::Ingress;
use kube::{Api, Config, Error};
use kube::api::{DeleteParams, ListParams, Meta, PostParams, WatchEvent};
use kube::Client;
use kube::config::{Kubeconfig, KubeConfigOptions};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

mod templates;
pub mod crd;
pub mod ingress;


pub fn from_kubeconfig(kubeconfig_path: &Path) -> (Client, String) {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path).unwrap();
    let config: Config = block_on(Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())).unwrap();
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client: Client = Client::new(config);
    return (client, kubeconfig_namespace);
}

pub fn try_default() -> Result<(Client, String), Error> {
    let config = block_on(Config::infer())?;
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client = Client::new(config);
    return Result::Ok((client, kubeconfig_namespace));
}

/// Deployment descriptor - contains deployment specification as defined by the user/called
/// and list if Kubernetes entities deployed, if any.
#[derive(Serialize, Deserialize, Debug)]
pub struct Deployment {
    pub specification: DeploymentSpecification,
    pub ingresses: Vec<Ingress>,
    pub stateful_sets: Vec<StatefulSet>,
    pub services: Vec<Service>,
}

impl Deployment {
    /// Deployment might contain a specification, yet it might not contain any deployed units yet.
    pub fn new(specification: DeploymentSpecification) -> Self {
        Deployment { specification, services: vec!(), ingresses: vec!(), stateful_sets: vec!() }
    }
}

/// Deployment as specified by the user. Not all values might be explicitly inserted by the user,
/// some might originate from defaults - it is assumed user willingly chose the defaults.
#[derive(Serialize, Deserialize, Debug)]
pub struct DeploymentSpecification {
    /// Name of the deployment. If not provided by the user, the value is randomly generated.
    pub name: String,
    /// Namespace to deploy to.
    pub namespace: String,
    /// Memory percentage to allocate by the JVM running H2O inside the docker container.
    pub memory_percentage: u8,
    /// Total memory for each H2O node. Effectively a pod memory request and limit.
    pub memory: String,
    /// Number of CPUs allocated for each H2O node. Effectively a pod CPU request and limit.
    pub num_cpu: u32,
    /// Total count of H2O nodes inside the cluster created.
    pub num_h2o_nodes: u32,
    /// Kubeconfig - provided optionally. There are well-known standardized locations to look for Kubeconfig, therefore optional.
    pub kubeconfig_path: Option<PathBuf>,
}

impl DeploymentSpecification {
    pub fn new(name: String, namespace: String, memory_percentage: u8, memory: String, num_cpu: u32, num_h2o_nodes: u32, kubeconfig_path: Option<PathBuf>) -> Self {
        DeploymentSpecification { name, namespace, memory_percentage, memory, num_cpu, num_h2o_nodes, kubeconfig_path }
    }
}

/// Deploys an H2O cluster using the given `client` and `deployment_specification`.
pub async fn deploy_h2o_cluster(client: Client, deployment_specification: DeploymentSpecification) -> Result<Deployment, Error> {
    let mut deployment: Deployment = Deployment::new(deployment_specification);

    deployment.services.push(deploy_service(  client.clone(),&deployment).await?);
    deployment.stateful_sets.push(deploy_statefulset(client.clone(), &deployment).await?);

    // TODO: Rollback when there is an error at this point, not in the inner methods
    return Ok(deployment);
}

async fn deploy_service(client: Client, deployment: &Deployment) -> Result<Service, Error> {
    let service_api: Api<Service> = Api::namespaced(client.clone(), &deployment.specification.namespace);

    let service: Service = templates::h2o_service(&deployment.specification.name, &deployment.specification.namespace);
    return match service_api.create(&PostParams::default(), &service).await {
        Ok(service) => {
            Ok(service)
        }
        Err(e) => {
            eprintln!("Unable to deploy service for '{}' deployment. Rewinding existing deployment. Reason:\n{:?}", &deployment.specification.name, e);
            undeploy_h2o(&client, &deployment).unwrap();
            Err(e)
        }
    };
}

async fn deploy_statefulset(client: Client, deployment: &Deployment) -> Result<StatefulSet, Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), &deployment.specification.namespace);
    let stateful_set: StatefulSet = templates::h2o_stateful_set(&deployment.specification.name, &deployment.specification.namespace, "h2oai/h2o-open-source-k8s", "latest",
                                                                deployment.specification.num_h2o_nodes, deployment.specification.memory_percentage, &deployment.specification.memory, deployment.specification.num_cpu);
    return match statefulset_api.create(&PostParams::default(), &stateful_set).await {
        Ok(statefulset) => {
            Result::Ok(statefulset)
        }
        Err(e) => {
            eprintln!("Unable to statefulset for '{}' deployment. Rewinding existing deployment. Reason:\n{:?}", &deployment.specification.name, e);
            undeploy_h2o(&client, &deployment).unwrap();
            Result::Err(e)
        }
    };
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

pub fn deploy_ingress(client: &Client, deployment: &mut Deployment) -> Result<(), Error> {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();

    let api: Api<Ingress> = Api::namespaced(client.clone(), &deployment.specification.namespace);
    let ingress_template: Ingress = templates::h2o_ingress(&deployment.specification.name, &deployment.specification.namespace);

    return match tokio_runtime.block_on(api.create(&PostParams::default(), &ingress_template)) {
        Ok(ingress) => {
            let ingress_name: String = ingress.meta().name.as_ref().unwrap().clone();
            let mut created_ingress: Ingress = ingress;
            let lp: ListParams = ListParams::default()
                .fields(&format!("metadata.name={}", &ingress_name))
                .timeout(3);
            let mut event_stream = tokio_runtime.block_on(api.watch(&lp, "0")).unwrap().boxed();

            while let Some(status) = tokio_runtime.block_on(event_stream.try_next()).unwrap() {
                match status {
                    WatchEvent::Modified(up_to_date_ingress) => {
                        created_ingress = up_to_date_ingress;
                        if ingress::any_ip(&created_ingress).is_some() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            deployment.ingresses.push(created_ingress);
            return Ok(());
        }
        Err(err) => {
            Err(err)
        }
    };
}

#[cfg(test)]
mod tests {
    extern crate tests_common;

    use std::path::PathBuf;

    use super::{Deployment, DeploymentSpecification};
    use super::kube::Client;

    use self::tests_common::kubeconfig_location_panic;

    #[test]
    fn test_from_kubeconfig() {
        let kubeconfig_location: PathBuf = kubeconfig_location_panic();
        super::from_kubeconfig(kubeconfig_location.as_path());
    }

    #[test]
    fn test_deploy_h2o() {
        let (client, namespace): (Client, String) = super::try_default().unwrap();
        let deployment_specification: DeploymentSpecification = DeploymentSpecification::new("h2o-k8s-test-cluster".to_string(), namespace,
                                                                                             80, "256Mi".to_string(), 2, 2, None);
        let mut deployment: Deployment = super::deploy_h2o_cluster(&client, deployment_specification).unwrap();
        assert_eq!(1, deployment.services.len());
        assert_eq!(1, deployment.stateful_sets.len());
        assert_eq!(0, deployment.ingresses.len());

        // Deploy ingress on top of existing deployment
        match super::deploy_ingress(&client, &mut deployment) {
            Ok(_) => {
                assert_eq!(1, deployment.ingresses.len());
            }
            Err(e) => {
                panic!("Test of ingress deployment failed. Reason: \n{}", e);
            }
        }
        let undeployment_result = super::undeploy_h2o(&client, &deployment);
        assert!(undeployment_result.is_ok());
    }
}
