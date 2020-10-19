// Module with common test resouces
use std::env;
use std::path::PathBuf;

pub const TEST_KUBECONFIG_ENVVAR: &str = "KUBECONFIG";

/// Returns a String with path to kubeconfig, if the environment variable TEST_KUBECONFIG_ENVVAR is set.
/// If the variable is not set, the method panics, as it is assumed the env var is required for the test to be
/// completed successfully.
pub fn kubeconfig_location_panic<'a>() -> PathBuf {
    let kubeconfig_path_var: String = match env::var(TEST_KUBECONFIG_ENVVAR) {
        Ok(var) => { var }
        Err(err) => {
            panic!("Environment variable {} not defined.\
            Unable to construct Kubernetes client. Error: {}", TEST_KUBECONFIG_ENVVAR, err);
        }
    };

    let kubeconfig_path = PathBuf::from(kubeconfig_path_var);

    if !kubeconfig_path.is_file() {
        panic!("The KUBECONFIG '{}' is not a file", kubeconfig_path.to_str().unwrap());
    } else {
        return kubeconfig_path;
    }
}
