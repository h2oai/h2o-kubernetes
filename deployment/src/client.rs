use std::path::Path;

use kube::{Client, Config};
use kube::config::{Kubeconfig, KubeConfigOptions};

use crate::Error;

/// Constructs a client from a kubeconfig to be found under the specified `kubeconfig` path.
/// Returns default namespace as a second value in the tuple.
/// # Arguments
///
/// `kubeconfig_path` - A valid path to an existing Kubeconfig
///
/// # Panics
///
/// If the `kubeconfig` path is malformed, does not exists or does not represent a valid Kubeconfig, this method panics.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use std::path::Path;
/// use kube::Client;
/// let path = Path::new("/etc/rancher/k3s/k3s.yaml");
/// let (client, namespace): (Client, String) = deployment::client::from_kubeconfig(&path).await
/// .expect("Client could not be created from Kubeconfig.");
/// }
/// ```
pub async fn from_kubeconfig(kubeconfig_path: &Path) -> Result<(Client, String), Error> {
    let kubeconfig: Kubeconfig = Kubeconfig::read_from(kubeconfig_path)?;
    let config: Config = Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())
        .await?;
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client: Client = Client::new(config);
    return Result::Ok((client, kubeconfig_namespace));
}

/// Attempts to construct a `kube::Client` by searching for the `KUBECONFIG` environment variable and possibly
/// other well-known places. If no kubeconfig is found, returns `Result::Err`. Returns default namespace as a second value in the tuple.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await
/// .expect("Could not construct client.");
/// }
/// ```
pub async fn try_default() -> Result<(Client, String), Error> {
    let config = Config::infer().await?;
    let kubeconfig_namespace: String = config.default_ns.clone();
    let client = Client::new(config);
    return Result::Ok((client, kubeconfig_namespace));
}