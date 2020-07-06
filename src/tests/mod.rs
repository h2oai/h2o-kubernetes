// Module with common test resouces
use std::env;

pub const TEST_KUBECONFIG_ENVVAR: &str = "KUBECONFIG";
pub const TEST_CLUSTER_NAMESPACE: &str = "default";

/// Returns a String with path to kubeconfig, if the environment variable TEST_KUBECONFIG_ENVVAR is set.
/// If the variable is not set, the method panics, as it is assumed the env var is required for the test to be
/// completed successfully.
pub fn kubeconfig_location_panic<'a>() -> String {
    return match env::var(TEST_KUBECONFIG_ENVVAR) {
        Ok(var) => { var }
        Err(err) => {
            panic!("Environment variable {} not defined.\
            Unable to construct Kubernetes client. Error: {}", TEST_KUBECONFIG_ENVVAR, err);
        }
    };
}