use k8s_openapi::api::core::v1::Service;
use kube::{Api, Client, Error};
use kube::api::{PostParams, DeleteParams};

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

pub fn h2o_service(name: &str, namespace: &str) -> Service {
    let service_definition: String = SERVICE_TEMPLATE.replace("<name>", name)
        .replace("<namespace>", namespace);

    let service: Service = serde_yaml::from_str(&service_definition).unwrap();
    return service;
}

pub async fn create(client: Client, namespace: &str, name: &str) -> Result<Service, Error> {
    let service_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let service: Service = h2o_service(name, namespace);
    return service_api.create(&PostParams::default(), &service).await;
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let statefulset_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let result = statefulset_api.delete(name, &DeleteParams::default()).await;

    return match result {
        Ok(_) => { return Ok(()); }
        Err(error) => { Err(error) }
    };
}