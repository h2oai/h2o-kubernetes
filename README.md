# H2O Kubernetes Operator

![Rust](https://github.com/h2oai/h2o-kubernetes/workflows/Rust/badge.svg)a

A command line tool to ease the deployment (and undeployment) of H2O open-source machine learning platform [H2O-3](https://github.com/h2oai/h2o-3) to Kubernetes. Currently in beta, with stable basic deployment/undeployment features. Official [H2O Kubernetes Docker images](https://hub.docker.com/r/h2oai/h2o-open-source-k8s) are used.

[Download for Mac/Linux/Windows](https://github.com/h2oai/h2o-kubernetes/releases).

## Usage

![H2O Usage in console](.img/h2o-operator.gif)

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

## Deployment
The operator is not officially released as a Docker container yet, nor is it available on OperatorHub or as a Red Hat Certified operator. In order to use it, a Docker container must be built first.

```shell
docker build --build-arg H2O_VERSION=custom -t h2o-operator:custom -f docker/public/Dockerfile-h2o-release build/
```
Once the Dockerfile is built, make sure to push it to a proper repository reachable by the Kubernetes cluster (e.g. [Docker Hub](hub.docker.com)).

Before the actual image is deployed, the H2O `CustomResourceDefinition` must be created in the Kubernetes cluster. The definition is to be found
in [bundle/manifests/h2o.crd.yaml](crd/h2os.h2o.ai.crd.yaml). Download it and do `kubectl apply -f h2os.h2o.ai.crd.yaml`. Such an operation requires
user with the following permissions:

```yaml
- apiGroups:
    - "apiextensions.k8s.io"
  resources:
    - customresourcedefinitions
  verbs:
    - create
```

Once the H2O CRD is deployed, the operator itself may also be deployed. A simple `Deployment` with exactly one instance of the pod
with H2O operator inside will do. In case of failures, the pod is restarted and H2O-related events are handled.
```
apiVersion: apps/v1
kind: Deployment
metadata:
  name: h2o-operator
  labels:
    app: h2o-operator
spec:
  replicas: 1
  selector:
    matchLabels:
      app: h2o-operator
  template:
    metadata:
      labels:
        app: h2o-operator
    spec:
      containers:
      - name: h2o-operator
        image: repository/h2o-operator:custom # Replace with a real docker image specification
        imagePullPolicy: Always # Set to IfNotPresent if the image with the very same tag never changes
```

The operator requires specific permissions to run, too. Make sure to use a Kubernetes `User` or create a dedicated `ServiceAccount`
with rights listed in the [ClusterRole definition file](tests/permissions/cluster_role.yaml). This set of permissions is used to test the operator itself.

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

## Contributing

Contributions are welcome and encouraged. Please refer to the [**contributing guide**](CONTRIBUTING.md). If you've encountered a bug,
or there is any feature missing, please create an [issue on GitHub](https://github.com/h2oai/h2o-kubernetes).

## License
This project is licensed under the [Apache License 2.0](LICENSE).

## Technical documentation

Technical documentation signpost is to be found in the [documentation](documentation/README.md) folder.
