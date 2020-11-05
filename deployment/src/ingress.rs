use kube::{Client, Api, Error};
use kube::api::{PostParams, DeleteParams};
use k8s_openapi::api::networking::v1beta1::Ingress;


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
          serviceName: <name>-service
          servicePort: 80
"#;

pub fn h2o_ingress(name: &str, namespace: &str) -> Ingress {
    let ingress_definition = INGRESS_TEMPLATE.replace("<name>", name)
        .replace("<namespace>", namespace);

    let ingress: Ingress = serde_yaml::from_str(&ingress_definition).unwrap();
    return ingress;
}

pub async fn create(client: Client, namespace: &str, name: &str) -> Result<Ingress, Error> {
    let api: Api<Ingress> = Api::namespaced(client, namespace);
    let ingress_template: Ingress = h2o_ingress(name, name);

    return api.create(&PostParams::default(), &ingress_template).await;
}

pub async fn delete(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Ingress> = Api::namespaced(client, namespace);
    let result = api.delete(name, &DeleteParams::default()).await;

    return match result {
        Ok(_) => { Ok(()) }
        Err(error) => { Err(error) }
    };
}

/// Returns the first IP assigned to an Ingress found, if found. Otherwise returns None.
pub fn any_ip(ingress: &Ingress) -> Option<String> {
    return ingress.status.as_ref()?
        .load_balancer.as_ref()?
        .ingress.as_ref()?
        .last()?
        .ip.clone();
}

/// Returns the first Path assigned to an Ingress found, if found. Otherwise returns None.
pub fn any_path(ingress: &Ingress) -> Option<String> {
    return ingress.spec.as_ref()?
        .rules.as_ref()?
        .last()?
        .http.as_ref()?
        .paths.last()?
        .path.clone();
}