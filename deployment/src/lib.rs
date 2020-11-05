extern crate futures;
extern crate kube;
extern crate log;

use std::path::Path;

use kube::{Config, Error};
use kube::Client;
use kube::config::{Kubeconfig, KubeConfigOptions};
use crate::crd::H2OSpec;

pub mod crd;
pub mod ingress;
pub mod service;
pub mod statefulset;
pub mod finalizer;


pub async fn from_kubeconfig(kubeconfig_path: &Path) -> (Client, String) {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path).unwrap();
    let config: Config = Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default()).await.unwrap();
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client: Client = Client::new(config);
    return (client, kubeconfig_namespace);
}

pub async fn try_default() -> Result<(Client, String), Error> {
    let config = Config::infer().await?;
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client = Client::new(config);
    return Result::Ok((client, kubeconfig_namespace));
}

/// Deploys an H2O cluster using the given `client` and `deployment_specification`.
pub async fn deploy_h2o_cluster(client: Client, specification: &H2OSpec, namespace: &str, name: &str) -> Result<(), Error> {
    let service_future = service::create(client.clone(), namespace, name);
    let statefulset_future = statefulset::create(client.clone(), specification, namespace, name);
    tokio::try_join!(service_future, statefulset_future)?;
    return Ok(());
}

pub async fn undeploy_h2o_cluster(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let service_future = service::delete(client.clone(), namespace, name);
    let statefulset_future = statefulset::delete(client.clone(), namespace, name);
    tokio::try_join!(service_future, statefulset_future)?;
    return Ok(());
}

#[cfg(test)]
mod tests {
    extern crate tests_common;

    use std::path::PathBuf;

    use super::kube::Client;

    use self::tests_common::kubeconfig_location_panic;
    use crate::crd::{H2OSpec, Resources};
    use k8s_openapi::api::apps::v1::StatefulSet;
    use kube::Api;
    use kube::api::ListParams;
    use k8s_openapi::api::core::v1::{Pod, Service};

    #[tokio::test]
    async fn test_from_kubeconfig() {
        let kubeconfig_location: PathBuf = kubeconfig_location_panic();
        super::from_kubeconfig(kubeconfig_location.as_path()).await;
    }

    #[tokio::test]
    async fn test_deploy_h2o_cluster() {
        let (client, namespace): (Client, String) = super::try_default().await.unwrap();
        let name: &str = "h2o-test";
        let resources: Resources = Resources::new(1, "256Mi".to_string(), Some(90));
        let specification: H2OSpec = H2OSpec::new(2, Option::Some("latest".to_string()), resources, Option::None);

        super::deploy_h2o_cluster(client.clone(), &specification, &namespace, name).await.unwrap();

        let ss_api: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
        assert_eq!(1, ss_api.list(&ListParams::default().labels(&format!("app={}", &name))).await.unwrap().items.len());
        let service_api: Api<Service> = Api::namespaced(client.clone(), &namespace);
        assert_eq!(1, service_api.list(&ListParams::default().labels(&format!("app={}", &name))).await.unwrap().items.len());

        super::undeploy_h2o_cluster(client.clone(), &namespace, name).await.unwrap();
    }
}
