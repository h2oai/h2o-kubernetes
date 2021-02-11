use kube::{Client, Api};
use k8s_openapi::api::core::v1::ConfigMap;
use std::collections::BTreeMap;
use k8s_openapi::ByteString;
use kube::api::{PostParams, ObjectMeta, DeleteParams};
use std::path::{Path};
use crate::Error;
use kube::client::Status;
use either::Either;

pub const H2O_CLUSTERING_JAR_PATH_KEY: &str = "H2O_CLUSTERING_JAR_PATH";
pub const H2O_CLUSTERING_CONFIG_MAP_NAME: &str = "h2o-clustering";
const H2O_CLUSTERING_JAR_FILE_NAME: &str = "h2o-clustering.jar";

pub async fn create_clustering_configmap(client: Client, namespace: &str, path: &Path) -> Result<ConfigMap, Error> {

    let api: Api<ConfigMap> = Api::namespaced(client, namespace);
    let bytes: ByteString = ByteString{ 0: std::fs::read(path).unwrap() };
    let mut binary_data: BTreeMap<String, ByteString> = BTreeMap::new();
    binary_data.insert(H2O_CLUSTERING_JAR_FILE_NAME.to_owned(), bytes);

    let config_map: ConfigMap = ConfigMap{
        binary_data: Some(binary_data),
        data: None,
        metadata: ObjectMeta{
            annotations: None,
            cluster_name: None,
            creation_timestamp: None,
            deletion_grace_period_seconds: None,
            deletion_timestamp: None,
            finalizers: None,
            generate_name: None,
            generation: None,
            labels: None,
            managed_fields: None,
            name: Some(H2O_CLUSTERING_CONFIG_MAP_NAME.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: None,
            resource_version: None,
            self_link: None,
            uid: None
        }
    };

    Ok(api.create(&PostParams::default(), &config_map).await?)
}

pub async fn exists(client: Client, namespace: &str) -> bool{
    let api: Api<ConfigMap> = Api::namespaced(client, namespace);
    let x = api.get(H2O_CLUSTERING_CONFIG_MAP_NAME).await;

    x.is_ok()
}

pub async fn delete(client: Client, namespace: &str) -> Result<Either<ConfigMap, Status>, Error>{
    let api: Api<ConfigMap> = Api::namespaced(client, namespace);
    let result = api.delete(H2O_CLUSTERING_CONFIG_MAP_NAME, &DeleteParams::default()).await;

    Ok(result?)
}

#[cfg(test)]
mod tests{
    use kube::{Client, Api};
    use crate::client;
    use std::env;
    use std::path::{PathBuf};
    use std::str::FromStr;
    use k8s_openapi::api::core::v1::ConfigMap;
    use crate::configmap::H2O_CLUSTERING_JAR_FILE_NAME;
    use kube::api::{DeleteParams, Meta};

    #[tokio::test]
    async fn test_configmap (){
        let (client, namespace): (Client, String) = client::try_default().await.unwrap();
        let clustering_jar_path_val: String = env::var(super::H2O_CLUSTERING_JAR_PATH_KEY).unwrap();
        let clustering_jar_path_buf:PathBuf = PathBuf::from_str(&clustering_jar_path_val).unwrap();

        super::create_clustering_configmap(client.clone(), &namespace, clustering_jar_path_buf.as_path()).await.unwrap();
        let api: Api<ConfigMap> = Api::namespaced(client.clone(), &namespace);
        let config_map : ConfigMap = api.get(super::H2O_CLUSTERING_CONFIG_MAP_NAME).await.unwrap();
        assert_eq!(namespace, config_map.namespace().unwrap());
        assert_eq!(1, config_map.binary_data.as_ref().unwrap().len());
        assert!(config_map.binary_data.unwrap().contains_key(H2O_CLUSTERING_JAR_FILE_NAME));
        api.delete(super::H2O_CLUSTERING_CONFIG_MAP_NAME, &DeleteParams::default()).await.unwrap();
    }

}