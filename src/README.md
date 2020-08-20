# Architecture

There are 4 modules:

1. main (the binary executable itself),
1. cli,
1. k8s,
1. test.

The `test` module only contains common code for unit tests inside other modules.
Integration tests are present in `{project-root}/tests` - a default location for 
`cargo` projects.

```
                           +------------------------+
                           |                        |
                           | H2OK Executable (main) |
                           |                        |
                           +-----+------------^-----+
                                 ^            |
   +------------------+          |            |      +---------------+
   |                  |          |            |      |               |
   |    K8S Module    +----------+            +------+  CLI Module   |
   |                  |                              |               |
   +------------------+                              +---------------+
Deploys H2O components to K8S.                   User-facing CLI. Validations, help, defaults.
Called by main executable after spec             Used by main executable to obtain cluster spec.
is gathered from the user.
```

The `main` module represents the `h2ok` binary and orchestrates other modules.
Deployment to Kubernetes and respective structures are separated into `k8s` module, as it is assumed
those could be re-used in future in other derived, such as REST client, offering the same services.
User input handling is separated in the `cli` module for the same reason.