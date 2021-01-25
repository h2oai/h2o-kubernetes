use deployment::crd::H2OSpec;
use kube::Client;

pub async fn check_h2o_cluster_integrity(client: Client, h2o_spec: &H2OSpec) -> bool{

    return cluster_healthy().await;
}

async fn cluster_healthy() -> bool{

    true
}

