FROM scratch

# Labels required by Red Hat certification
LABEL operators.operatorframework.io.bundle.mediatype.v1=registry+v1
LABEL operators.operatorframework.io.bundle.manifests.v1=manifests/
LABEL operators.operatorframework.io.bundle.metadata.v1=metadata/
LABEL operators.operatorframework.io.bundle.package.v1=h2o-operator
LABEL operators.operatorframework.io.bundle.channels.v1=beta
LABEL operators.operatorframework.io.bundle.channel.default.v1=beta
LABEL com.redhat.openshift.versions="v4.5,v4.6"
LABEL com.redhat.delivery.backport=true
LABEL com.redhat.delivery.operator.bundle=true


COPY operator/bundle/manifests /manifests/
COPY operator/bundle/metadata /metadata/