use kube::Client;

/// Shared state handed to every reconcile call.
pub struct Context {
    pub client: Client,
}
