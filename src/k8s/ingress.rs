use k8s_openapi::api::networking::v1beta1::Ingress;

/// Returns the first IP assigned to an Ingress found, if found. Otherwise returns None.
pub fn any_ip(ingress: &Ingress) -> Option<String> {
    return ingress.status.as_ref()?
        .load_balancer.as_ref()?
        .ingress.as_ref()?
        .last()?
        .ip.clone();
}

/// Returns the first Path assigned to an Ingress found, if found. Otherwise returns None.
pub fn any_path(ingress: &Ingress) -> Option<String> {
    return ingress.spec.as_ref()?
        .rules.as_ref()?
        .last()?
        .http.as_ref()?
        .paths.last()?
        .path.clone();
}