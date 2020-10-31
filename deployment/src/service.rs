use k8s_openapi::api::core::v1::Service;
use kube::{Api, Client, Error};
use kube::api::{PostParams, DeleteParams};

use crate::{Deployment};

const SERVICE_TEMPLATE: &str = r#"
apiVersion: v1
kind: Service
metadata:
  name: <name>
  namespace: <namespace>
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
    let service_definition = SERVICE_TEMPLATE.replace("<name>", name)
        .replace("<namespace>", namespace);

    let service: Service = serde_yaml::from_str(&service_definition).unwrap();
    return service;
}

pub async fn create(client: Client, deployment: &Deployment) -> Result<Service, Error> {
    let service_api: Api<Service> = Api::namespaced(client.clone(), &deployment.specification.namespace);
    let service: Service = h2o_service(&deployment.specification.name, &deployment.specification.namespace);
    return service_api.create(&PostParams::default(), &service).await;
}

pub async fn delete(client: Client, name: &str, namespace: &str) {
    let statefulset_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    statefulset_api.delete(name, &DeleteParams::default()).await.unwrap();
}