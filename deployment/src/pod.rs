use std::collections::HashMap;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::{Pod};
use kube::{Api, Client};
use kube::api::{DeleteParams, ListParams, Meta, PostParams};
use kube::client::Status;
use kube_runtime::watcher::Event;
use log::debug;

use crate::crd::H2OSpec;
use crate::Error;

pub const H2O_DEFAULT_PORT: u16 = 54321;
pub const H2O_CLUSTERING_PORT: u16 = 8080;


const POD_TEMPLATE: &str = r#"
apiVersion: v1
kind: Pod
metadata:
  name: <name>
  namespace: <namespace>
  labels:
    app: <deployment-label>
spec:
  containers:
    - name: <name>
      image: '<h2o-image>'
      volumeMounts:
        - name: h2o-clustering-volume
          mountPath: /opt/h2o-clustering
<command-line>
      ports:
        - containerPort: 54321
          protocol: TCP
        - containerPort: 54322
          protocol: TCP
        - containerPort: 8080
          protocol: TCP
      resources:
        limits:
          cpu: '<num-cpu>'
          memory: <memory>
        requests:
          cpu: '<num-cpu>'
          memory: <memory>
      env:
      - name: H2O_ASSISTED_CLUSTERING_API_PORT
        value: '8080'
      - name: H2O_ASSISTED_CLUSTERING_REST
        value: 'True'
  volumes:
    - name: h2o-clustering-volume
      configMap:
        # Provide the name of the ConfigMap containing the files you want
        # to add to the container
        name: h2o-clustering
  restartPolicy: Never
"#;

/// Creates a `Pod` object with H2O docker container inside. The `POD_TEMPLATE`
/// yaml template from this module is used and populated with arguments of this function.
///
/// # Arguments
/// `name` - Name of this specific pod
/// `deployment_label` - Name of the H2O cluster/deployment this pod belongs to. Set as a label
/// on the `Pod` created by this function.
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
/// use k8s_openapi::api::core::v1::Pod;
/// use deployment::pod::h2o_pod;
/// let pod: Pod = h2o_pod(
/// "some-pod-name",
/// "some-h2o-deployment-name",
/// "default",
/// "h2oai/h2o-open-source-k8s:latest",
/// Option::None,
/// 3,
/// "32Gi",
/// 8
/// )
/// .expect("Could not create H2O Pod from YAML template");
/// ```
pub fn h2o_pod(
    name: &str,
    deployment_label: &str,
    namespace: &str,
    docker_image: &str,
    command: Option<&str>,
    nodes: u32,
    memory: &str,
    num_cpu: u32,
) -> Result<Pod, Error> {
    let mut command_line: String = "      command: <command>".to_string(); // with proper indentation
    match command {
        None => command_line = "".to_string(),
        Some(custom_command) => {
            command_line = command_line.replace("<command>", custom_command);
        }
    }

    let pod_yaml_definition: String = POD_TEMPLATE
        .replace("<name>", name)
        .replace("<deployment-label>", deployment_label)
        .replace("<namespace>", namespace)
        .replace("<h2o-image>", docker_image)
        .replace("<command-line>", &command_line)
        .replace("<nodes>", &nodes.to_string())
        .replace("<memory>", memory)
        .replace("<num-cpu>", &num_cpu.to_string());

    debug!("Stateful set result:\n{}", pod_yaml_definition);

    let stateful_set: Pod = serde_yaml::from_str(&pod_yaml_definition)?;
    return Ok(stateful_set);
}

pub async fn create_pods(client: Client, h2o_spec: &H2OSpec, deployment_name: &str, namespace: &str) -> Result<Vec<Pod>, Vec<Error>> {
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let post_params: PostParams = PostParams::default();

    let mut official_image_temp: String = String::from("h2oai/h2o-open-source-k8s:");
    let docker_image: &str;
    let command_string: String;
    let command: Option<&str>;


    // Custom image has the priority and overrides H2O version specified. In case both custom image and version are specified.
    if let Some(image) = h2o_spec.custom_image.as_ref() {
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
    } else if h2o_spec.version.is_some() {
        official_image_temp.push_str(h2o_spec.version.as_ref().unwrap());
        docker_image = &official_image_temp;

        command_string = format!(r#"["/bin/bash", "-c", "java -XX:+UseContainerSupport -XX:MaxRAMPercentage={} -cp /opt/h2oai/h2o-3/h2o.jar:/opt/h2o-clustering/h2o-clustering.jar water.H2OApp"]"#,
                                 h2o_spec.resources.memory_percentage.unwrap_or(50)); // Must be saved to a String with the same lifetime as the optional command
        command = Option::Some(&command_string);
    } else {
        // At least one of the above has to be specified - H2O version that serves as a Docker image tag,
        // or a full definition of custom image.
        return Err(vec!(Error::UserError("Unable to create H2O Pods. Either H2O version or a complete custom image specification must be provided. None provided."
            .to_string())));
    }

    // Pods are created concurrently (or directly in parallel, as long as the chosen runtime is multi-threaded one) in a similar way to StatefulSet's parallel pod management.
    // It might take a while to spawn a pod. Waiting for previous pod in order to spawn the next one prolongs the waiting times.
    // Especially for large clusters, this ensures fastest startup time possible.
    let pod_creation_results: Vec<Result<Pod, Error>> = futures::stream::iter(0..h2o_spec.nodes)
        .map(|pod_number| {
            let pod_name: String = format!("{}-{}", deployment_name, pod_number);
            let h2o_pod: Pod = h2o_pod(&pod_name, deployment_name, namespace,
                                       docker_image, command, h2o_spec.nodes, &h2o_spec.resources.memory,
                                       h2o_spec.resources.cpu,
            ).unwrap();
            create_pod(h2o_pod, &api, &post_params)
        }).buffer_unordered(h2o_spec.nodes as usize) // Order of invocation and completion is irrelevant.
        .map_err(Error::from)
        .collect()
        .await;

    // Filter out pods that were not deployed successfully
    let erroneous_pods_count: usize = pod_creation_results.iter()
        .filter(|res| {
            res.is_err()
        })
        .count();

    // If any pod creation ended up with an error, roll back the successfully deployed ones and return errors gathered.
    if erroneous_pods_count > 0 {
        let successfully_deployed_pods: Vec<&Pod> = pod_creation_results.iter()
            .filter_map(|res| {
                res.as_ref().ok()
            })
            .collect();

        let delete_params: Arc<DeleteParams> = Arc::new(DeleteParams::default());
        let _: Vec<Result<(), Error>> = futures::stream::iter(0..successfully_deployed_pods.len())
            .map(|idx| {
                let pod = successfully_deployed_pods[idx];
                delete_pod(pod.metadata.name.as_ref().expect("Pods are supposed to have names."), &api, &delete_params)
            }).buffer_unordered(successfully_deployed_pods.len())
            .collect()
            .await;

        return Err(pod_creation_results.into_iter()
            .filter_map(Result::err)
            .collect());
    }

    return Ok(pod_creation_results.into_iter()
        .filter_map(Result::ok)
        .collect());
}


async fn delete_pods(client: Client, namespace: &str, pod_names: &[&str]) -> Vec<Result<either::Either<Pod, Status>, Error>> {
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let delete_params: DeleteParams = DeleteParams::default();

    futures::stream::iter(pod_names)
        .map(|pod_name| {
            api.delete(pod_name, &delete_params)
        }).buffer_unordered(pod_names.len())
        .map_err(Error::from)
        .collect()
        .await
}


async fn create_pod(pod: Pod, api: &Api<Pod>, params: &PostParams) -> Result<Pod, kube::Error> {
    let future = api.create(&params, &pod);
    return future.await;
}

async fn delete_pod(pod_name: &str, api: &Api<Pod>, params: &DeleteParams) -> Result<(), Error> {
    let future = api.delete(&pod_name, &params);
    future.await?;
    Ok(())
}

pub async fn wait_pod_status<F>(client: Client, pod_label: &str, namespace: &str, expected_count: usize, pod_status_check: F) -> Vec<Pod>
    where F: Fn(&Pod) -> bool {
    let api: Api<Pod> = Api::<Pod>::namespaced(client.clone(), namespace);
    let list_params: ListParams = ListParams::default()
        .labels(&format!("app={}", pod_label));

    let mut pod_events = kube_runtime::watcher(api, list_params).boxed();
    let mut discovered_pods: HashMap<String, Pod> = HashMap::with_capacity(expected_count);

    'podloop: while let Some(result) = pod_events.next().await {
        match result {
            Ok(event) => {
                match event {
                    Event::Applied(pod) => {
                        if pod_status_check(&pod) {
                            discovered_pods.insert(pod.name().clone(), pod);
                            if discovered_pods.len() == expected_count {
                                break 'podloop;
                            }
                        }
                    }
                    Event::Deleted(_) => {}
                    Event::Restarted(pods) => {
                        for pod in pods {
                            if pod_status_check(&pod) {
                                discovered_pods.insert(pod.name().clone(), pod);
                                if discovered_pods.len() == expected_count {
                                    break 'podloop;
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
    };

    // Pods do not support `Eq` for HashSets, return as plain vector
    let pods = discovered_pods.values().map(|entry| {
        entry.clone()
    }).collect::<Vec<Pod>>();

    return pods;
}

pub async fn wait_pods_deleted(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list_params: ListParams = ListParams::default()
        .labels(&format!("app={}", name));

    let mut pod_count: usize = pod_api.list(&pod_list_params).await.unwrap().items.len();
    debug!("Waiting to delete {} pods.", pod_count);
    if pod_count == 0 { return Result::Ok(()); }

    let mut stream = kube_runtime::watcher(pod_api, pod_list_params).boxed();
    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                match event {
                    Event::Deleted(_) => {
                        pod_count = pod_count - 1;
                        if pod_count == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    };
    return Result::Ok(());
}

pub fn get_pod_ip(pod: &Pod) -> String {

    if let Some(status) = &pod.status{
        if let Some(ip) = status.pod_ip.as_ref(){
            return format!("{} : {}", pod.metadata.name.clone().unwrap_or("Unnamed_pod".to_owned()), ip);
        }

        if pod.metadata.name.is_some(){
            return pod.metadata.name.clone().unwrap();
        }
    }

    "Unknown pod name with unknown IP.".to_owned()
}

pub async fn delete_pods_label(client: Client, namespace: &str, label: &str){
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let pods_list_params: ListParams = ListParams::default()
        .labels(&format!("app={}", label));
    let x = api.delete_collection(&DeleteParams::default(), &pods_list_params).await;
}


#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Pod;
    use kube::Client;

    use tests_common::kubeconfig_location_panic;

    use crate::crd::{H2OSpec, Resources};
    use crate::pod::wait_pod_status;

    #[tokio::test]
    async fn test_create_pods() {
        let kubeconfig_location = kubeconfig_location_panic();
        let (client, namespace): (Client, String) = crate::client::from_kubeconfig(kubeconfig_location.as_path())
            .await
            .unwrap();

        // Create H2O in Kubernetes cluster
        let h2o_name = "test-create-pods";
        let node_count: usize = 1;
        let resources: Resources = Resources::new(1, "256Mi".to_string(), Option::None);
        let h2o_spec: H2OSpec = H2OSpec::new(node_count as u32, Option::Some("latest".to_string()), resources, Option::None);

        // Create pods according to the H2OSpec created above
        let created_pods = super::create_pods(client.clone(), &h2o_spec, &h2o_name, &namespace)
            .await
            .expect("Expected pods to be deployed correctly.");
        assert_eq!(h2o_spec.nodes as usize, created_pods.len());

        // Wait for all the pods to be created and check their count
        let verified_pods: Vec<Pod> = wait_pod_status(client.clone(), h2o_name, &namespace, node_count,
                                                        |pod| { pod.metadata.creation_timestamp.is_some() }).await;
        assert_eq!(h2o_spec.nodes as usize, verified_pods.len());

        let deleted_pod_names: Vec<&str> = created_pods.iter()
            .map(|pod| {
                pod.metadata.name.as_ref()
                    .expect("Pod's name is expected to be mandatory.")
                    .as_ref()
            })
            .collect();

        super::delete_pods(client.clone(), &namespace, deleted_pod_names.as_slice()).await;
        super::wait_pods_deleted(client.clone(), h2o_name, &namespace).await.expect("Pods are supposed to be deleted.");
    }
}