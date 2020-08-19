# H2O Kubernetes CLI

![Rust](https://github.com/h2oai/h2o-kubernetes/workflows/Rust/badge.svg)

A command-line tool to ease deployment (and undeployment) of H2O open-source machine learning platform [H2O-3](https://github.com/h2oai/h2o-3) to Kubernetes. Currently in a state of a proof-of-concept. Official [H2O Kubernetes Docker images](https://hub.docker.com/r/h2oai/h2o-open-source-k8s) are used.

![H2O Usage in console](h2ok.gif)

## Usage
Type `h2ok --help` for an overview of available subcommands. Use the `--help` or `-h` flag in combination with any of the subcommands to receive help for those subcommands, for example `h2ok deploy -h`.

There are two basic commands:
1. `h2ok deploy`,
1. `h2ok undeploy`.

### Deploy
Deploys an H2O cluster into Kubernetes by creating all the necessary components. Once successfully deployed a deployment descriptor file with cluster name is saved. Such a file can be used to undeploy the H2O cluster or built on top of by adding additional services.
If deployment of any of the component fails a rollback of existing components is attempted automatically.
 
**Mininal example**: `h2ok deploy --cluster-size 3`.
**Minimal example - custom kubeconfig and namespace**: `h2ok deploy --cluster-size 3 --kubeconfig /etc/rancher/k3s/k3s.yaml --namespace default`
The `namespace` option defaults to namespace. If `kubeconfig` is not defined, well-known locations and environment variables are searched.

After each deployment is done, a file with cluster's name and an `.h2ok` suffix is saved to the working directory. Such a file serves as a descriptor of the deployment done and may later be used by `h2ok undeploy -f h2o-deployment-name.h2ok` to automatically undeploy the whole H2O cluster from Kubernetes.

### Undeploy
Undeploys existing deployment from a Kubernetes cluster using deployment descriptor generated during deployment operations.

**Minimal example**: `h2ok undeploy -f h2ok-deployment-name.h2ok`

## The future plans
- Support deployment of the whole machine learning toolkit for easy bootstrap, e.g. deploy Jupyter notebook and expose it.
   Currently, only basic H2O-3 deployment is supported.
- Define version of H2O to deploy
- Custom H2O-3 docker image & custom repository
- External XGBoost support

Goal is to provide a fully configurable tool with reasonable defaults for everyday use.

## Building, testing and running

H2O Kubernetes CLI (`h2ok`) is written in [Rust](https://www.rust-lang.org/), using its standard built-in tools. The build and dependency management tool is therefore [Cargo](https://crates.io/).

- Development build : `cargo build`
- Release build: `cargo release`
- Development run: `cargo run -- deploy --namespace default --kubeconfig /etc/rancher/k3s/k3s.yaml --cluster_size 3`
- Test: `cargo test` - please note many tests have prerequisities - running Kubernetes cluster and the `KUBECONFIG` variable set.

## Automated tests
Automated tests are run via GitHub actions - the test environemnt provides the `KUBECONFIG` environment variable with path to a [K3S](https://k3s.io/) Kubernetes instance.
