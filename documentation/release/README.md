# Releasing

Release is done using GitHub actions. The release action for H2O Operator is defined in the [release-operator.yml](../../../.github/workflows/release-operator.yml)
There are following stages:

1. Run a complete battery of tests,
1. Build distributable operator binary,
1. Create and push Docker images,
1. Create GitHub release and version tags,
1. Increment versions using [SemVer2](https://semver.org/) (conditions apply).
1. Create patch branch (conditions apply)

## 1. Tests battery

A full battery of tests is ran. Currently, it is the same set of automated tests used regularly for pull requests and pushes. Automated
tests and tests in general are described in greater detail in it's own [section](../tests/README.md). As the release is done
both for general audience and OpenShift specific-users, both `v1` and `v1beta` versions of the `CustomResourceDefinition` are tested.

## 2. Distributable operator binary
Unlike CLI, operator is intended to run inside a Docker container. Therefore, only `amd64` linux generic binary is built to be 
appended to the GitHub Release created later. This binary is intended to be used inside custom Docker containers as a primary way
to deploy operators into Kubernetes cluster. 

## 3. Create and push Docker images

Docker images are pushed into:
1. [H2O](https://hub.docker.com/repository/docker/h2oai/h2o-k8s-operator) Docker Hub,
1. Red Hat registry for certification.

Credentials are stored as GitHub secrets.

For both Red Hat and Docker Hub, an identical image is pushed. This means the [Dockerfile](../../docker/Dockerfile-operator) is identical. For Red Hat, an additional image named `operator bundle image` with metadata is pushed.
The image and the binary inside are therefore using exactly the same codebase. In addition to Docker Hub, Red Hat [certifies](https://connect.redhat.com/en/partner-with-us/red-hat-openshift-operator-certification)
the operator. 

The operator image is based on UBI - a necessary prerequisite in order for the image to be certified on Red Hat. Newest versions of
dependencies like `openssl` are installed using `microdnf` inside the [Dockerfile](../../docker/Dockerfile-operator). Afterwards,
 H2O Operator is compiled from scratch using those dependencies. This ensures best image security rating (also called "health index" by Red Hat), as all dependencies are up to date.

### Red Hat OpenShift process

There are two images pushed to OpenShift: the `operator image` and the `operator bundle` image. The operator image contains
the operator binary and runs it, as specified in the [Dockerfile](../../docker/Dockerfile-operator). The bundle image contains
metadata and instructions on how to install the operator into the OpenShift Kubernetes cluster. These files to be found in
the [bundle](../../bundle) folder include (not an exhaustive list):

1. A `CSV` file - stands for `ClusterServiceVersion`,
1. The H2O `CustomResourceDefinition`,
1. Operator and custom resource version metadata,
1. Licensing.

The operator image always has to be deployed, certified and published **first**, followed by the operator [bundle image](../../docker/Dockerfile-operator-bundle),
as the operator is actually deployed for tests as the bundle image is verified. The bundle image's `CSV` file actually points
to the corresponding operator image version. The `<version>` placeholder is replaced using `sed` at release time in the
[release-operator.yml](../../../.github/workflows/release-operator.yml) action. So is `<creation-date>`. H2O is released separately from the
[H2O-3](https://github.com/h2oai/h2o-3) repository, as its release cycle is different to operator's. The exact process of operator
release in Red Hat is as follows:

1. Build `operator` docker image,
1. Push `operator` docker image into the Red Hat provided [operator repository](https://connect.redhat.com/project/5929091/view),
1. Check Red Hat Rest API for validation errors,
1. If there are no errors and the validation is done, trigger the `publish` action on `operator` image using the same REST API,
1. Build `bundle` image,
1. Push bundle image into the Red Hat provided [bundle repository](https://connect.redhat.com/project/5929221/view),
1. Check Red Hat REST API for validation errors,
1. If there are no errors, `publish` the `bundle` image using REST API.

If any of the above-mentioned steps fails, the operator is not released properly and leftovers have to be cleaned manually.
The process is described in the official [documentation](https://redhat-connect.gitbook.io/partner-guide-for-red-hat-openshift-and-container/) - requires account
to access. If you're an H2O employee, please ask in the `#devops-requests` Slack channel for access. There is also a separate documentation for the
[REST API](https://connect.redhat.com/api-docs#/).

The validations checks and publishing is **automated** using the [red_hat_docker_certification.py](../../release/red_hat_docker_certification.py) script.
This script checks given docker image in Red Hat scan repository for validation outcome. If successful, triggers `publish` action. There 
is no timeout in the script itself, as the timeout of this job is set directly in the [release-operator.yml](../../../.github/workflows/release-operator.yml) action.
Further documentation is to be found in the script itself.


**Certification note:** The Certification may take up to 4 hours officially. For one image. The time variance is observed to be huge. From several
minutes to tens of hours (definitely less than 24h). This is potentially a common point of failure.

### Docker Hub push
Docker hub doesn't do any validations or docker image checks, the image is simply pushed to Docker hub into the [h2oai](https://hub.docker.com/u/h2oai) space.
Login credentials are stored in this GitHub repository using GitHub secrets. This action is only ran after the Red Hat step succeeds.

## 4. GitHub Release

Part of the release is a tag in the following format: `operator-x.y.z`, where `x.y.z` is the SemVer2 version of the operator released. The following
files are appended to the release:

1. Linux generic amd64 binary with the operator,
1. H2O `CustomResourceDefinition` files, both version `v1` and `v1beta` (version of the CRD definition, not a H2O version of H2O resource),
1. A file with `ClusterRole` definition for easy permission setup for Kubernetes administrators when deployed manually.

## 5. Increment version
When released from `master` branch, the version in that branch is updated in [Cargo.toml](../../Cargo.toml) according to user's input and
then committed to the `master` branch.
When release is done from any other branch (patch branches, but other names are allowed too), the `patch` part of operator version
is incremented by `1` in [Cargo.toml](../../Cargo.toml) and committed into the branch.

## 6. Create patch branch
When release from `master`, a new branch named `operator-patch-x.y.z`, where `x.y.z` is the version of the operator released,
only with the `patch` part incremented by `1` (according to SemVer2). Any further patches for that particular major version should go into
and be released from this newly created branch.

When **not** released from master, no branch is created.