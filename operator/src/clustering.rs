use k8s_openapi::api::core::v1::Pod;
use kube::Client;

async fn cluster_pods(client: Client, namespace: &str, pod_label: &str, expected_pod_count: usize) {
    let pod_has_ip_check: fn(&Pod) -> bool = |pod| {
        if let Some(status) = pod.status.as_ref() {
            return status.pod_ip.is_some()
        }
        false
    };

    let created_pods = deployment::pod::wait_pods_created(client.clone(), pod_label, namespace,
                                                          expected_pod_count as usize,
                                                          pod_has_ip_check
    ).await;

    let pod_ips: Vec<String> = created_pods.into_iter()
        .map(|pod|{
            pod.status
                .expect("Pod expected to have a status entry, as this had been checked before.")
                .pod_ip
                .expect("Pod expected to have a pod IP assigned, as this had been checked before.")
        })
        .collect();

}

async fn send_flatfile(pod_ips: &[&str]){
}

#[cfg(test)]
mod test{

    #[tokio::test]
    async fn test_cluster_pods(){
    }
}