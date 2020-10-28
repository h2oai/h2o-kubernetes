use k8s_openapi::api::core::v1::Service;

const SERVICE_TEMPLATE: &str = r#"
apiVersion: v1
kind: Service
metadata:
  name: <name>-service
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