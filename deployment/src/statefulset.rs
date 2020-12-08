use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{Api, Client};
use kube::api::{DeleteParams, PostParams, PropagationPolicy};
use log::debug;

use crate::crd::H2OSpec;
use crate::Error;

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

/// Creates an H2O `StatefulSet` object from given parameters for further deployment into Kubernetes cluster
/// from a YAML template.
///
/// # Arguments
/// `name` - Name of the H2O deployment. Also used to label the resources.
/// `namespace` - Namespace the resources belong to - used in resources metadata.
/// `docker_image` - The Docker image with H2O to use
/// `command` - Custom command for the `docker_image` with H2O
/// `nodes` - Number of H2O nodes to spown - translated to a number of pods/replicas in a statefulset.
/// `memory` - Amount of memory limits and requests for each pod. These are set to equal values in order
/// for H2O to be reproducible. Kubernetes-compliant string expected.
/// `num_cpu` - Number of virtual CPUs for each pod (and therefore each H2O node). Same value is set to
/// both requests and limits to ensure reproducibility of H2O's operations.
///
/// # Examples
///
/// ```no_run
///     use k8s_openapi::api::apps::v1::StatefulSet;
/// use deployment::statefulset::h2o_stateful_set;
/// let stateful_set: StatefulSet = h2o_stateful_set(
/// "any-name",
/// "default",
/// "h2oai/h2o-open-source-k8s:latest",
/// Option::None,
/// 3,
/// "32Gi",
/// 8
/// )
/// .expect("Could not create StatefulSet from YAML template");
/// ```
pub fn h2o_stateful_set(
    name: &str,
    namespace: &str,
    docker_image: &str,
    command: Option<&str>,
    nodes: u32,
    memory: &str,
    num_cpu: u32,
) -> Result<StatefulSet, Error> {
    let mut command_line: String = "          command: <command>".to_string(); // with proper indentation
    match command {
        None => command_line = "".to_string(),
        Some(custom_command) => {
            command_line = command_line.replace("<command>", custom_command);
        }
    }

    let stateful_set_definition = STATEFUL_SET_TEMPLATE
        .replace("<name>", name)
        .replace("<namespace>", namespace)
        .replace("<h2o-image>", docker_image)
        .replace("<command-line>", &command_line)
        .replace("<nodes>", &nodes.to_string())
        .replace("<memory>", memory)
        .replace("<num-cpu>", &num_cpu.to_string());

    debug!("Stateful set result:\n{}", stateful_set_definition);

    let stateful_set: StatefulSet = serde_yaml::from_str(&stateful_set_definition)?;
    return Ok(stateful_set);
}

/// Invokes asynchronous creation of `StatefulSet` of H2O pods in a Kubernetes cluster according to the specification.
///
/// # Arguments
/// `client` - Client to create the StatefulSet with
/// `specification` - Specification of the H2O cluster
/// `namespace` - namespace to deploy the statefulset to
/// `name` - Name of the statefulset, used for statefulset and pod labeling as well.
pub async fn create(
    client: Client,
    specification: &H2OSpec,
    namespace: &str,
    name: &str,
) -> Result<StatefulSet, Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let mut official_image_temp: String = String::from("h2oai/h2o-open-source-k8s:");
    let docker_image: &str;
    let command_string: String;
    let command: Option<&str>;


    // Custom image overrides H2O version in case both is specified
    if let Some(image) = specification.custom_image.as_ref() {
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
        command_string = format!(r#"["/bin/bash", "-c", "java -XX:+UseContainerSupport -XX:MaxRAMPercentage={} -jar /opt/h2oai/h2o-3/h2o.jar"]"#,
                                 specification.resources.memory_percentage.unwrap_or(50)); // Must be saved to a String with the same lifetime as the optional command
        command = Option::Some(&command_string);
    } else {
        // At least one of the above has to be specified - H2O version that serves as a Docker image tag,
        // or a full definition of custom image.
        return Err(Error::UserError("Unable to create H2O statefulset. Either H2O version or a complete custom image specification must be provided. None obtained."
            .to_string()));
    }

    let stateful_set: StatefulSet = h2o_stateful_set(
        name,
        namespace,
        docker_image,
        command,
        specification.nodes,
        &specification.resources.memory,
        specification.resources.cpu,
    )?;

    let statefulset : StatefulSet = statefulset_api
        .create(&PostParams::default(), &stateful_set)
        .await?;
    Ok(statefulset)
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
/// deployment::statefulset::delete(client, &namespace, "any-h2o-name").await.unwrap();
/// }
/// ```
pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let statefulset_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let delete_params: DeleteParams = DeleteParams {
        dry_run: false,
        grace_period_seconds: None,
        propagation_policy: Some(PropagationPolicy::Foreground),
        preconditions: None,
    };

    statefulset_api.delete(name, &delete_params).await?;
    Ok(())
}
