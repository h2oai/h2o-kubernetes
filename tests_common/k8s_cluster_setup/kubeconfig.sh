#!/bin/bash
# This script is a modified version of the original to be found at https://github.com/ddiiwoong/kubeconfig-generator
set -e
set -o pipefail

# Add user to k8s using service account, the RBAC must be created separately
if [[ -z "$1" ]] || [[ -z "$2" ]]; then
 echo "usage: $0 <service_account_name> <namespace>"
 exit 1
fi
 
SERVICE_ACCOUNT_NAME=$1
NAMESPACE="$2"
KUBECFG_FILE_NAME="kubeconfigs-generated/kubeconfig-${SERVICE_ACCOUNT_NAME}-${NAMESPACE}.yaml"
TARGET_FOLDER="kubeconfigs-generated"
 
create_target_folder() {
    echo -n "Creating target directory to hold files in ${TARGET_FOLDER}..."
    mkdir -p "${TARGET_FOLDER}"
    printf "done"
}
 
create_service_account() {
    echo -e "\\nCreating a service account: ${SERVICE_ACCOUNT_NAME} on namespace: ${NAMESPACE}"
    kubectl create sa "${SERVICE_ACCOUNT_NAME}" --namespace "${NAMESPACE}"
}
 
get_secret_name_from_service_account() {
    echo -e "\\nGetting secret of service account ${SERVICE_ACCOUNT_NAME}-${NAMESPACE}"
    SECRET_NAME=$(kubectl get sa "${SERVICE_ACCOUNT_NAME}" --namespace "${NAMESPACE}" -o json | jq -r '.secrets[].name')
    echo "Secret name: ${SECRET_NAME}"
}
 
extract_ca_crt_from_secret() {
    echo -e -n "\\nExtracting ca.crt from secret..."
    kubectl get secret "${SECRET_NAME}" --namespace "${NAMESPACE}" -o json | jq \
    -r '.data["ca.crt"]' | base64 -d > "${TARGET_FOLDER}/ca.crt"
    printf "done"
}
 
get_user_token_from_secret() {
    echo -e -n "\\nGetting user token from secret..."
    USER_TOKEN=$(kubectl get secret "${SECRET_NAME}" \
    --namespace "${NAMESPACE}" -o json | jq -r '.data["token"]' | base64 -d)
    printf "done"
}
 
set_kube_config_values() {
    context=$(kubectl config current-context)
    echo -e "\\nSetting current context to: $context"
 
    CLUSTER_NAME=$(kubectl config get-contexts "$context" | awk '{print $3}' | tail -n 1)
    echo "Cluster name: ${CLUSTER_NAME}"
 
    ENDPOINT=$(kubectl config view \
    -o jsonpath="{.clusters[?(@.name == \"${CLUSTER_NAME}\")].cluster.server}")
    echo "Endpoint: ${ENDPOINT}"
 
    # Set up the config
    echo -e "\\nPreparing kubeconfig-${SERVICE_ACCOUNT_NAME}-${NAMESPACE}.yaml"
    echo -n "Setting a cluster entry in kubeconfig..."
    kubectl config set-cluster "${CLUSTER_NAME}" \
    --kubeconfig="${KUBECFG_FILE_NAME}" \
    --server="${ENDPOINT}" \
    --certificate-authority="${TARGET_FOLDER}/ca.crt" \
    --embed-certs=true
 
    echo -n "Setting token credentials entry in kubeconfig..."
    kubectl config set-credentials \
    "${SERVICE_ACCOUNT_NAME}-${NAMESPACE}-${CLUSTER_NAME}" \
    --kubeconfig="${KUBECFG_FILE_NAME}" \
    --token="${USER_TOKEN}"
 
    echo -n "Setting a context entry in kubeconfig..."
    kubectl config set-context \
    "${SERVICE_ACCOUNT_NAME}-${NAMESPACE}-${CLUSTER_NAME}" \
    --kubeconfig="${KUBECFG_FILE_NAME}" \
    --cluster="${CLUSTER_NAME}" \
    --user="${SERVICE_ACCOUNT_NAME}-${NAMESPACE}-${CLUSTER_NAME}" \
    --namespace="${NAMESPACE}"
 
    echo -n "Setting the current-context in the kubeconfig file..."
    kubectl config use-context "${SERVICE_ACCOUNT_NAME}-${NAMESPACE}-${CLUSTER_NAME}" \
    --kubeconfig="${KUBECFG_FILE_NAME}"
}

create_rbac() {
    export service_account_name=${SERVICE_ACCOUNT_NAME}
    export namespace=${NAMESPACE}
    rm -f final.yml temp.yml  
    ( echo "cat <<EOF >final.yml";
    cat $(dirname $0)/crb.tmpl;
    echo "EOF";
    ) >temp.yml
    . temp.yml
    kubectl create -f final.yml
    echo -e "############# review rbac manifest ##############\n"
    cat final.yml
    rm -f final.yml temp.yml
}

create_target_folder
create_service_account
get_secret_name_from_service_account
extract_ca_crt_from_secret
get_user_token_from_secret
set_kube_config_values
  

echo -n "Creating RBAC"
create_rbac

echo -e "\\nAll done!"
