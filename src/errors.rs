#[derive(Debug, thiserror::Error)]
pub enum VirtualLbError {
    #[error("Kubernetes API error: {0}")]
    Kube(#[from] kube::Error),

    // TODO: can this ever happen??
    #[error("Service '{0}' has no namespace")]
    MissingNamespace(String),
}
