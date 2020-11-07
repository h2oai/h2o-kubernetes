# H2O Kubernetes

![Rust](https://github.com/h2oai/h2o-kubernetes/workflows/Rust/badge.svg)

Repository with official tools to aid the deployment of [H2O Machine Learning platform](https://github.com/h2oai/h2o-3) to [Kubernetes](https://kubernetes.io/).
There are two essential tools to be found in this repository:

1. **H2O Operator** - for first class H2O Kubernetes support ([README](cli/README.md)),
1. **Command Line Interface** - to ease deployment of the operator and/or deploy H2O to clusters without the operator ([README](operator/README.md)).

Binaries available: [**Download for Mac / Linux / Windows**](https://github.com/h2oai/h2o-kubernetes/releases).
Or [build from source](CONTRIBUTING.md).

![operator](.img/h2o-operator.gif)

The **operator** is an implementation of [Kubernetes operator pattern](https://kubernetes.io/docs/concepts/extend-kubernetes/operator/)
specifically for H2O. Once deployed to a Kubernetes cluster, a new custom resource named `H2O` is recognized by Kubernetes,
making it easy to create H2O clusters inside Kubernetes cluster using plain `kubectl`. The **CLI** is a binary usually running on the client's
side, usable to deploy the operator itself into Kubernetes cluster or create H2O clusters in Kubernetes in cases when the **operator**
itself may not be used. There are also [Helm charts](https://charts.h2o.ai/) available as yet another way to deploy H2O into Kubernetes.
Using the operator first and then falling back to CLI/Helm is the recommended approach.

For detailed instructions on how to use each tool, please refer to the specific user guides:

- [CLI](cli/README.md)
- [OPERATOR](operator/README.md)

## Contributing

Contributions are welcome and encouraged. Please refer to the [**contributing guide**](CONTRIBUTING.md). If you've encountered a bug,
or there is any feature missing, please create an [issue on GitHub](https://github.com/h2oai/h2o-kubernetes).

## License
This project is licensed under the [Apache License 2.0](LICENSE).