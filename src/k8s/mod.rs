extern crate futures;
extern crate kube;

use std::path::Path;

use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::extensions::v1beta1::Ingress;
use kube::Client;
use tokio::runtime::Runtime;

use self::futures::executor::block_on;
use self::kube::{Api, Config};
use self::kube::api::PostParams;
use self::kube::config::{Kubeconfig, KubeConfigOptions};

mod definitions;

pub fn from_kubeconfig(kubeconfig_path: &Path) -> Client {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path).unwrap();
    let config: Config = block_on(Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())).unwrap();
    let client = Client::new(config);
    return client;
}

pub fn deploy_h2o(client: Client, namespace: &str) {
    let mut tokio_runtime: Runtime = tokio::runtime::Runtime::new().unwrap();
    let ingress_api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let ingress = definitions::h2o_ingress("h2o-k8s", namespace);
    tokio_runtime.block_on(ingress_api.create(&PostParams::default(), &ingress)).unwrap();

    let service_api: Api<_> = Api::namespaced(client.clone(), namespace);

    let service: Service = definitions::h2o_service("h2o-k8s", namespace);
    tokio_runtime.block_on(service_api.create(&PostParams::default(), &service)).unwrap();

    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let stateful_set = definitions::h2o_stateful_set("h2o-k8s", namespace, "h2oai/h2o-open-source-k8s", "latest");
    tokio_runtime.block_on(statefulset_api.create(&PostParams::default(), &stateful_set)).unwrap();
    println!("H2O started.");
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;
    use super::kube::Client;

    const TEST_KUBECONFIG_ENVVAR: &str = "KUBECONFIG";
    const TEST_CLUSTER_NAMESPACE: &str = "default";

    #[test]
    fn test_from_kubeconfig() {
        let kubeconfig_location: String = match env::var(TEST_KUBECONFIG_ENVVAR) {
            Ok(var) => { var },
            Err(err) => {
                panic!("Environment variable {} not defined.\
            Unable to construct Kubernetes client. Error: {}", TEST_KUBECONFIG_ENVVAR, err);
            },
        };

        let kubeconfig_path: &Path = Path::new(&kubeconfig_location);
        assert!(kubeconfig_path.exists());
        super::from_kubeconfig(kubeconfig_path);
    }

    #[test]
    fn test_deploy_h2o() {
        let kubeconfig_location: String = match env::var(TEST_KUBECONFIG_ENVVAR) {
            Ok(var) => { var },
            Err(err) => {
                panic!("Environment variable {} not defined.\
            Unable to construct Kubernetes client. Error: {}", TEST_KUBECONFIG_ENVVAR, err);
            },
        };

        let kubeconfig_path: &Path = Path::new(&kubeconfig_location);
        assert!(kubeconfig_path.exists());
        let client: Client = super::from_kubeconfig(kubeconfig_path);

        super::deploy_h2o(client, TEST_CLUSTER_NAMESPACE)
    }
}