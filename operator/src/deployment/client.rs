use kube::{Client, Config};
use kube::config::{KubeConfigOptions};

use crate::Error;

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