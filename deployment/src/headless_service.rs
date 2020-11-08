use k8s_openapi::api::core::v1::Service;
use kube::{Api, Client};
use kube::api::{DeleteParams, PostParams};

use crate::Error;

const SERVICE_TEMPLATE: &str = r#"
apiVersion: v1
kind: Service
metadata:
  name: <name>
  namespace: <namespace>
  labels:
    app: <name>
spec:
  type: ClusterIP
  clusterIP: None
  selector:
    app: <name>
  ports:
  - protocol: TCP
    port: 80
    targetPort: 54321
"#;

/// Creates an H2O `Service` object from given parameters for further deployment into Kubernetes cluster
/// from a YAML template.
///
/// # Arguments
/// `name` - Name of the Service. Typically corresponds to the rest of H2O deployment Also used to label the service.
/// `namespace` - Namespace the services belongs to.
///
/// # Examples
///
/// ```no_run
/// use k8s_openapi::api::core::v1::Service;
/// let service: Service = deployment::headless_service::h2o_service(
/// "any-name",
/// "default"
/// )
/// .expect("Could not create service from YAML template.");
/// ```
pub fn h2o_service(name: &str, namespace: &str) -> Result<Service, Error> {
    let service_definition: String = SERVICE_TEMPLATE
        .replace("<name>", name)
        .replace("<namespace>", namespace);

    let service: Service = serde_yaml::from_str(&service_definition)
        .map_err(Error::from_serde_yaml_error)?;
    return Ok(service);
}

/// Invokes asynchronous creation of a headless `Service`.
///
/// # Arguments
/// `client` - Client to create the Service with
/// `namespace` - namespace to deploy the Service to
/// `name` - Name of the service, used to label the service instance as well
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use k8s_openapi::api::core::v1::Service;
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// let service: Service = deployment::headless_service::create(client, &namespace, "any-name").await.unwrap();
/// }
/// ```
pub async fn create(client: Client, namespace: &str, name: &str) -> Result<Service, Error> {
    let service_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let service: Service = h2o_service(name, namespace)?;
    return service_api.create(&PostParams::default(), &service).await
        .map_err(Error::from_kube_error);
}

/// Invokes asynchronous deletion of a `StatefulSet` of H2O pods from a Kubernetes cluster.
///
/// # Arguments
///
/// `client` - Client to delete the statefulset with
/// `namespace` - Namespace to delete the statefulset from. User is responsible to provide
/// correct namespace. Otherwise `Result::Err` is returned.
/// `name` - Name of the statefulset to invoke deletion for.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
/// use kube::Client;
/// let (client, namespace): (Client, String) = deployment::client::try_default().await.unwrap();
/// deployment::headless_service::delete(client, &namespace, "any-name").await.unwrap();
/// }
/// ```
pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let statefulset_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let result = statefulset_api.delete(name, &DeleteParams::default()).await
        .map_err(Error::from_kube_error);

    return match result {
        Ok(_) => {
            return Ok(());
        }
        Err(error) => Err(error),
    };
}
