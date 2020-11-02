use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{Api, Client, Error};
use kube::api::{DeleteParams, PostParams, PropagationPolicy};

use crate::Deployment;

const STATEFUL_SET_TEMPLATE: &str = r#"
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: <name>
  namespace: <namespace>
spec:
  serviceName: <name>
  podManagementPolicy: "Parallel"
  replicas: <nodes>
  selector:
    matchLabels:
      app: <name>
  template:
    metadata:
      labels:
        app: <name>
    spec:
      containers:
        - name: <name>
          image: '<docker-img-name>:<docker-img-tag>'
          command: ["/bin/bash", "-c", "java -XX:+UseContainerSupport -XX:MaxRAMPercentage=<memory-percentage> -jar /opt/h2oai/h2o-3/h2o.jar"]
          ports:
            - containerPort: 54321
              protocol: TCP
          readinessProbe:
            httpGet:
              path: /kubernetes/isLeaderNode
              port: 8081
            initialDelaySeconds: 5
            periodSeconds: 5
            failureThreshold: 1
          resources:
            limits:
              cpu: '<num-cpu>'
              memory: <memory>
            requests:
              cpu: '<num-cpu>'
              memory: <memory>
          env:
          - name: H2O_KUBERNETES_SERVICE_DNS
            value: <name>.<namespace>.svc.cluster.local
          - name: H2O_NODE_LOOKUP_TIMEOUT
            value: '180'
          - name: H2O_NODE_EXPECTED_COUNT
            value: '<nodes>'
          - name: H2O_KUBERNETES_API_PORT
            value: '8081'
"#;

pub fn h2o_stateful_set(name: &str, namespace: &str, docker_img_name: &str, docker_img_tag: &str, nodes: u32,
                        memory_percentage: u8, memory: &str, num_cpu: u32) -> StatefulSet {
    let stateful_set_definition = STATEFUL_SET_TEMPLATE.replace("<name>", name)
        .replace("<namespace>", namespace)
        .replace("<docker-img-name>", docker_img_name)
        .replace("<docker-img-tag>", docker_img_tag)
        .replace("<nodes>", &nodes.to_string())
        .replace("<memory-percentage>", &memory_percentage.to_string())
        .replace("<memory>", memory)
        .replace("<num-cpu>", &num_cpu.to_string());

    let stateful_set: StatefulSet = serde_yaml::from_str(&stateful_set_definition).unwrap();
    return stateful_set;
}

pub async fn create(client: Client, deployment: &Deployment) -> Result<StatefulSet, Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), &deployment.specification.namespace);
    let stateful_set: StatefulSet = h2o_stateful_set(&deployment.specification.name, &deployment.specification.namespace, "h2oai/h2o-open-source-k8s", "latest",
                                                     deployment.specification.num_h2o_nodes, deployment.specification.memory_percentage, &deployment.specification.memory, deployment.specification.num_cpu);

    return statefulset_api.create(&PostParams::default(), &stateful_set).await;
}

pub async fn delete(client: Client, name: &str, namespace: &str) {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let delete_params: DeleteParams = DeleteParams {
        dry_run: false,
        grace_period_seconds: None,
        propagation_policy: Some(PropagationPolicy::Foreground),
        preconditions: None,
    };
    statefulset_api.delete(name, &delete_params).await.unwrap();
}
