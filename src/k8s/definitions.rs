use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::extensions::v1beta1::Ingress;
use serde_yaml;

const STATEFUL_SET_TEMPLATE: &str = r#"
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: <name>-stateful-set
  namespace: <namespace>
spec:
  serviceName: h2o-service
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
            value: <name>-service.<namespace>.svc.cluster.local
          - name: H2O_NODE_LOOKUP_TIMEOUT
            value: '180'
          - name: H2O_NODE_EXPECTED_COUNT
            value: '<nodes>'
          - name: H2O_KUBERNETES_API_PORT
            value: '8081'
"#;

pub fn h2o_stateful_set(name: &str, namespace: &str, docker_img_name: &str, docker_img_tag: &str, nodes: i32,
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

const INGRESS_TEMPLATE: &str = r#"
apiVersion: extensions/v1beta1
kind: Ingress
metadata:
  name: <name>-ingress
spec:
  rules:
  - http:
      paths:
      - path: /
        pathType: Prefix
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