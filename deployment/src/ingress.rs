use k8s_openapi::api::networking::v1beta1::Ingress;
use kube::{Api, Client};
use kube::api::{DeleteParams, PostParams};

use crate::Error;

const INGRESS_TEMPLATE: &str = r#"
apiVersion: networking.k8s.io/v1beta1
kind: Ingress
metadata:
  name: <name>
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /$2
    traefik.frontend.rule.type: PathPrefixStrip
spec:
  rules:
  - http:
      paths:
      - path: /<name>
        pathType: Exact
        backend:
          serviceName: <name>
          servicePort: 80
"#;

/// Creates an H2O `Ingress` targeting a service of the same `name` to be further deployed into a Kubernetes
/// cluster. It is assumed the servicePort is 80 and the target port is 54321 (the default H2O port).
///
/// # Arguments
/// `name` - Name of the H2O deployment. Also used to label the the ingress.
/// `namespace` - Namespace the ingress will be created in.
fn h2o_ingress(name: &str, namespace: &str) -> Result<Ingress, Error> {
    let ingress_definition = INGRESS_TEMPLATE
        .replace("<name>", name)
        .replace("<namespace>", namespace);

    let ingress: Ingress = serde_yaml::from_str(&ingress_definition)
        .map_err(Error::from_serde_yaml_error)?;
    return Ok(ingress);
}

/// Invokes asynchronous creation of an `Ingress`.
///
///
/// # Arguments
/// `client` - Client to create the StatefulSet with
/// `specification` - Specification of the H2O cluster
/// `namespace` - namespace to deploy the statefulset to
/// `name` - Name of the statefulset, used for statefulset and pod labeling as well.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use k8s_openapi::api::networking::v1beta1::Ingress;
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// let ingress: Ingress = deployment::ingress::create(client, &namespace, "any-name").await.unwrap();
/// }
/// ```
pub async fn create(client: Client, namespace: &str, name: &str) -> Result<Ingress, Error> {
    let api: Api<Ingress> = Api::namespaced(client, namespace);
    let ingress_template: Ingress = h2o_ingress(name, namespace)?;

    return api.create(&PostParams::default(), &ingress_template).await
        .map_err(Error::from_kube_error);
}

/// Invokes asynchronous deletion of an `Ingress` from a Kubernetes cluster.
///
/// # Arguments
///
/// `client` - Client to delete the Ingress with
/// `namespace` - Namespace to delete the Ingress from. User is responsible to provide
/// correct namespace. Otherwise `Result::Err` is returned.
/// `name` - Name of the Ingress to invoke deletion for.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// deployment::ingress::delete(client, &namespace, "any-name").await.unwrap();
/// }
/// ```
pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let api: Api<Ingress> = Api::namespaced(client, namespace);
    let result = api.delete(name, &DeleteParams::default()).await
        .map_err(Error::from_kube_error);

    return match result {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    };
}

/// Returns the first IP assigned to an Ingresses load balancer, if found. Otherwise returns `Option::None`.
///
/// # Arguments
///
/// `ingress` - Ingress to search for IP
pub fn any_lb_external_ip(ingress: &Ingress) -> Option<String> {
    return ingress
        .status
        .as_ref()?
        .load_balancer
        .as_ref()?
        .ingress
        .as_ref()?
        .last()?
        .ip
        .clone();
}

/// Returns the first Path assigned to an Ingress found, if found. Otherwise returns None.
///
/// # Arguments:
/// `ingress` - Ingress to search for Path
pub fn any_path(ingress: &Ingress) -> Option<String> {
    return ingress
        .spec
        .as_ref()?
        .rules
        .as_ref()?
        .last()?
        .http
        .as_ref()?
        .paths
        .last()?
        .path
        .clone();
}
