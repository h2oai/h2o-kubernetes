# H2O Kubernetes Operator

![Rust](https://github.com/h2oai/h2o-kubernetes/workflows/Rust/badge.svg)

A command line tool to ease the deployment (and undeployment) of H2O open-source machine learning platform [H2O-3](https://github.com/h2oai/h2o-3) to Kubernetes. Currently in beta, with stable basic deployment/undeployment features. Official [H2O Kubernetes Docker images](https://hub.docker.com/r/h2oai/h2o-open-source-k8s) are used.

[Download for Mac/Linux/Windows](https://github.com/h2oai/h2o-kubernetes/releases).

## Usage

![H2O Usage in console](../.img/h2o-operator.gif)

Once the `h2o-operator` is running, an `H2O` resource can be used together with `kubectl`to deploy H2O into Kubernetes.
Example `h2o.yaml` YAML:

```yaml
apiVersion: h2o.ai/v1
kind: H2O
metadata:
  name: h2o-test
spec:
  nodes: 3
  version: "3.32.0.1"
  resources:
    cpu: 1
    memory: "512Mi"
    memoryPercentage: 90
```

After creating the resource by using`kubectl apply -f h2o.yaml`, all the necessary H2O resources are created.
Deletion is as simple as `kubectl delete h2o h2o-test`.

## Building, testing and running

Refer to the [contributing guide](../CONTRIBUTING.md) for detailed instructions on how to build and develop this project.

- Development build : `cargo build -p h2o-operator`
- Release build: `cargo build -p h2o-operator --release`
- Development run: `cargo run -p h2o-operator -- deploy --namespace default --kubeconfig /etc/rancher/k3s/k3s.yaml --cluster_size 3 -- version latest`
- Test: `cargo test -p h2o-operator` - please note many tests have prerequisites - running Kubernetes cluster and the `KUBECONFIG` variable set.

On start, the `h2o-operator` binary will look for `KUBECONFIG` variable. If such variable exists and points to a valid 
kubeconfig file with enough permissions to run the operator, it will try to detect or create the H2O custom resource inside 
the Kubernetes cluster and start serving requests. It can be deployed either directly into Kubernetes inside a Pod or ran
externally.