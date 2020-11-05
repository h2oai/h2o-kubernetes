use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{Api, Client, Error};
use kube::api::{DeleteParams, PostParams, PropagationPolicy};
use log::debug;

use crate::crd::{H2OSpec, CustomImage};

const STATEFUL_SET_TEMPLATE: &str = r#"
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: <name>
  namespace: <namespace>
  labels:
    app: <name>
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
          image: '<h2o-image>'
<command-line>
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

pub fn h2o_stateful_set(name: &str, namespace: &str, docker_image: &str, command: Option<&str>, nodes: u32,
                        memory_percentage: u8, memory: &str, num_cpu: u32) -> StatefulSet {
    let mut command_line: String = "          command: <command>".to_string(); // with proper indentation
    match command {
        None => { command_line = "".to_string() }
        Some(custom_command) => {
            command_line = command_line.replace("<command>", custom_command);
        }
    }

    let stateful_set_definition = STATEFUL_SET_TEMPLATE.replace("<name>", name)
        .replace("<namespace>", namespace)
        .replace("<h2o-image>", docker_image)
        .replace("<command-line>", &command_line)
        .replace("<nodes>", &nodes.to_string())
        .replace("<memory-percentage>", &memory_percentage.to_string())
        .replace("<memory>", memory)
        .replace("<num-cpu>", &num_cpu.to_string());

    debug!("Stateful set result:\n{}", stateful_set_definition);

    let stateful_set: StatefulSet = serde_yaml::from_str(&stateful_set_definition).unwrap();
    return stateful_set;
}

pub async fn create(client: Client, specification: &H2OSpec, namespace: &str, name: &str) -> Result<StatefulSet, Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let mut official_image_temp: String = String::from("h2oai/h2o-open-source-k8s:");
    let docker_image: &str;
    let command: Option<&str>;

    // Custom image overrides H2O version in case both is specified
    if specification.custom_image.is_some() {
        let image: &CustomImage = specification.custom_image.as_ref().unwrap();
        docker_image = &image.image;
        // The user optionally sets a custom entrypoint to be used for the custom image. If no
        match &image.command {
            None => {
                command = Option::None;
            }
            Some(custom_command) => {
                command = Option::Some(custom_command);
            }
        }
    } else if specification.version.is_some() {
        official_image_temp.push_str(specification.version.as_ref().unwrap());
        docker_image = &official_image_temp;
        command = Option::Some(r#"["/bin/bash", "-c", "java -XX:+UseContainerSupport -XX:MaxRAMPercentage=<memory-percentage> -jar /opt/h2oai/h2o-3/h2o.jar"]"#);
    } else {
        // At least one of the above has to be specified - H2O version that serves as a Docker image tag,
        // or a full definition of custom image.
        return Result::Err(Error::InvalidMethod("".to_string())); // TODO: Proper error
    }

    let stateful_set: StatefulSet = h2o_stateful_set(name, namespace, docker_image, command,
                                                     specification.nodes, specification.resources.memory_percentage.unwrap_or(50),
                                                     &specification.resources.memory, specification.resources.cpu);


    return statefulset_api.create(&PostParams::default(), &stateful_set).await;
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let delete_params: DeleteParams = DeleteParams {
        dry_run: false,
        grace_period_seconds: None,
        propagation_policy: Some(PropagationPolicy::Foreground),
        preconditions: None,
    };

    return match statefulset_api.delete(name, &delete_params).await {
        Ok(_) => { Ok(()) }
        Err(error) => { Err(error) }
    };
}
